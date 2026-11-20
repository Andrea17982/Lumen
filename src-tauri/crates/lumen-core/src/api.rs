//! Official HTTPS APIs. Secrets stay in the OS keychain; budget reservations survive crashes.
use crate::{models::*, providers::Invocation};
use futures_util::StreamExt;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use tokio::sync::watch;

const SERVICE: &str = "io.github.andrea17982.lumen.api";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub model: String,
    pub auto_enabled: bool,
    pub budget_usd: f64,
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub max_output_tokens: u32,
    pub rates_confirmed: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub config: Config,
    pub key_saved: bool,
    pub spent_usd: f64,
}
fn db(root: &Path) -> Result<Connection, String> {
    let c =
        Connection::open(root.join("api.sqlite")).map_err(|_| "API budget database unavailable")?;
    c.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "API budget database busy")?;
    c.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS config(account TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS charges(id TEXT PRIMARY KEY,account TEXT NOT NULL,model TEXT NOT NULL,micros INTEGER NOT NULL,status TEXT NOT NULL);").map_err(|_|"API budget database unavailable")?;
    Ok(c)
}
fn config(root: &Path, id: &str) -> Result<Config, String> {
    let raw: Option<String> = db(root)?
        .query_row("SELECT value FROM config WHERE account=?", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| "Cannot read API settings")?;
    raw.map(|s| serde_json::from_str(&s).map_err(|_| "Invalid API settings".into()))
        .unwrap_or(Ok(Config::default()))
}
fn spent(c: &Connection, id: &str) -> Result<i64, String> {
    c.query_row(
        "SELECT coalesce(sum(micros),0) FROM charges WHERE account=?",
        [id],
        |r| r.get(0),
    )
    .map_err(|_| "Cannot read API budget".into())
}
fn entry(id: &str) -> Result<keyring::Entry, String> {
    uuid::Uuid::parse_str(id).map_err(|_| "Invalid API account")?;
    keyring::Entry::new(SERVICE, id).map_err(|_| "OS keychain unavailable".into())
}
pub fn has_key(id: &str) -> Result<bool, String> {
    match entry(id)?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(_) => Err("OS keychain is locked or unavailable".into()),
    }
}
pub fn forget_key(id: &str) -> Result<(), String> {
    match entry(id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Cannot remove the API key from the OS keychain".into()),
    }
}
fn key(id: &str) -> Result<String, String> {
    entry(id)?
        .get_password()
        .map_err(|_| "Save the API key in Accounts; the OS keychain must be unlocked".into())
}
pub fn eligible(root: &Path, a: &Account) -> bool {
    config(root, &a.id).is_ok_and(|c| {
        c.auto_enabled
            && validate(&c).is_ok()
            && has_key(&a.id).unwrap_or(false)
            && db(root)
                .and_then(|d| spent(&d, &a.id))
                .is_ok_and(|s| s < money(c.budget_usd).unwrap_or(0))
    })
}
pub(crate) fn root(config: &Path) -> Result<&Path, String> {
    config
        .parent()
        .and_then(Path::parent)
        .ok_or("Invalid API account directory".into())
}
fn money(v: f64) -> Result<i64, String> {
    if !v.is_finite() || v < 0.0 || v > 1_000_000.0 {
        return Err("Use a finite non-negative USD amount up to 1,000,000".into());
    }
    Ok((v * 1_000_000.0).ceil() as i64)
}
fn validate(c: &Config) -> Result<(), String> {
    if c.model.is_empty()
        || c.model.len() > 100
        || !c
            .model
            .chars()
            .all(|x| x.is_ascii_alphanumeric() || "-._:/[]".contains(x))
    {
        return Err("Choose an exact API model ID".into());
    }
    if !c.rates_confirmed
        || money(c.budget_usd)? <= 0
        || !(256..=32768).contains(&c.max_output_tokens)
    {
        return Err("Confirm this model's official rates, a positive budget and an output limit between 256 and 32768".into());
    }
    money(c.input_per_million)?;
    money(c.output_per_million)?;
    Ok(())
}
fn cost(c: &Config, input: u64, output: u64) -> Result<i64, String> {
    let a = (input as u128)
        .checked_mul(money(c.input_per_million)? as u128)
        .ok_or("API cost overflow")?;
    let b = (output as u128)
        .checked_mul(money(c.output_per_million)? as u128)
        .ok_or("API cost overflow")?;
    i64::try_from((a + b).div_ceil(1_000_000)).map_err(|_| "API cost overflow".into())
}
fn reserve(
    root: &Path,
    id: &str,
    model: &str,
    input: u64,
    output_limit: (u32, u32),
    automatic: bool,
) -> Result<(String, Config, i64), String> {
    let mut d = db(root)?;
    let tx = d
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| "API budget is busy")?;
    let raw: String = tx
        .query_row("SELECT value FROM config WHERE account=?", [id], |r| {
            r.get(0)
        })
        .map_err(|_| "Configure API pricing and budget in Accounts")?;
    let c: Config = serde_json::from_str(&raw).map_err(|_| "Invalid API settings")?;
    validate(&c)?;
    if automatic && !c.auto_enabled {
        return Err("API use in Auto is disabled".into());
    }
    if c.model != model {
        return Err("API pricing belongs to another model. Update its rates in Accounts before switching models.".into());
    }
    if c.max_output_tokens != output_limit.0 || !(1..=output_limit.0).contains(&output_limit.1) {
        return Err(
            "API output limit changed while preparing the request; retry with the saved settings"
                .into(),
        );
    }
    let amount = cost(&c, input, output_limit.1 as u64)?;
    if spent(&tx, id)?
        .checked_add(amount)
        .ok_or("API budget overflow")?
        > money(c.budget_usd)?
    {
        return Err(
            "API budget exhausted or insufficient for this request's upper-bound reservation"
                .into(),
        );
    }
    let request = crate::models::id();
    tx.execute(
        "INSERT INTO charges VALUES(?,?,?,?, 'reserved')",
        params![request, id, model, amount],
    )
    .map_err(|_| "Cannot reserve API budget")?;
    tx.commit().map_err(|_| "Cannot reserve API budget")?;
    Ok((request, c, amount))
}
#[crate::command]
pub fn get_api_settings(core: crate::CoreRef<'_>, id: String) -> Result<State, String> {
    let a = crate::account(&core, &id)?;
    if !a.provider.is_api() {
        return Err("Not an API account".into());
    }
    Ok(State {
        config: config(&core.root, &id)?,
        key_saved: has_key(&id)?,
        spent_usd: spent(&db(&core.root)?, &id)? as f64 / 1e6,
    })
}
#[crate::command]
pub fn save_api_settings(
    core: crate::CoreRef<'_>,
    id: String,
    value: Config,
) -> Result<(), String> {
    let a = crate::account(&core, &id)?;
    if !a.provider.is_api() {
        return Err("Not an API account".into());
    }
    validate(&value)?;
    db(&core.root)?.execute("INSERT INTO config VALUES(?,?) ON CONFLICT(account) DO UPDATE SET value=excluded.value",params![&id,serde_json::to_string(&value).map_err(|_|"Invalid API settings")?]).map_err(|_|"Cannot save API settings")?;
    core.catalogs
        .lock()
        .map_err(|_| "Catalog lock failed")?
        .remove(&id);
    Ok(())
}
#[crate::command]
pub fn save_api_key(core: crate::CoreRef<'_>, id: String, secret: String) -> Result<(), String> {
    let a = crate::account(&core, &id)?;
    if !a.provider.is_api() {
        return Err("Not an API account".into());
    }
    let secret = secret.trim();
    if secret.is_empty() || secret.len() > 4096 || !secret.bytes().all(|x| x.is_ascii_graphic()) {
        return Err("Invalid API key format".into());
    }
    entry(&id)?
        .set_password(secret)
        .map_err(|_| "Cannot save the API key in the OS keychain".into())
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|_| "HTTPS client unavailable".into())
}
fn base(p: &Provider) -> String {
    #[cfg(debug_assertions)]
    if std::env::var_os("LUMEN_TEST_DATA_DIR").is_some() {
        if let Ok(raw) = std::env::var("LUMEN_TEST_API_BASE") {
            if let Ok(url) = reqwest::Url::parse(&raw) {
                if url.scheme() == "http"
                    && url.host_str() == Some("127.0.0.1")
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.path() == "/"
                    && url.query().is_none()
                    && url.fragment().is_none()
                {
                    return format!(
                        "{}{}/",
                        url,
                        serde_json::to_value(p).unwrap().as_str().unwrap()
                    );
                }
            }
        }
    }
    match p {
        Provider::Openaiapi => "https://api.openai.com/v1/",
        Provider::Anthropicapi => "https://api.anthropic.com/v1/",
        Provider::Googleapi => "https://generativelanguage.googleapis.com/v1beta/",
        _ => unreachable!(),
    }
    .into()
}

fn auth(r: reqwest::RequestBuilder, p: &Provider, key: &str) -> reqwest::RequestBuilder {
    match p {
        Provider::Openaiapi => r.bearer_auth(key),
        Provider::Anthropicapi => r
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01"),
        Provider::Googleapi => r.header("x-goog-api-key", key),
        _ => unreachable!(),
    }
}
async fn json_response(response: reqwest::Response) -> Result<Value, String> {
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16(){401=>"API authentication failed; check the key in Accounts".into(),403=>"API permission denied by the provider".into(),429=>format!("{}Official API rate limit or quota exhausted",crate::limits::RATE_LIMIT),_=>format!("Official API rejected the request (HTTP {}). Check model access and request limits.",status.as_u16())});
    }
    let mut bytes = vec![];
    let mut stream = response.bytes_stream();
    while let Some(b) = stream.next().await {
        let b = b.map_err(|_| "API response interrupted")?;
        if bytes.len() + b.len() > 8 * 1024 * 1024 {
            return Err("API response exceeds 8 MB".into());
        }
        bytes.extend_from_slice(&b)
    }
    serde_json::from_slice(&bytes).map_err(|_| "Official API returned invalid JSON".into())
}
pub async fn models(a: &Account, dir: &Path) -> Result<Vec<ModelInfo>, String> {
    let cfg = config(root(dir)?, &a.id)?;
    validate(&cfg)?;
    let secret = key(&a.id)?;
    let p = &a.provider;
    let mut url = format!("{}models", base(p));
    let http = client()?;
    let mut rows = vec![];
    for page in 0..20 {
        let response = json_response(
            auth(http.get(&url), p, &secret)
                .send()
                .await
                .map_err(|_| "Official API model catalog unavailable")?,
        )
        .await?;
        let items = response[if *p == Provider::Googleapi {
            "models"
        } else {
            "data"
        }]
        .as_array()
        .ok_or("API catalog has no model list")?;
        for m in items {
            let id = m[if *p == Provider::Googleapi {
                "name"
            } else {
                "id"
            }]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("models/");
            if id != cfg.model {
                continue;
            }
            if id.is_empty() {
                continue;
            }
            if *p == Provider::Googleapi
                && !m["supportedGenerationMethods"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x == "generateContent"))
            {
                continue;
            }
            rows.push(ModelInfo {
                id: id.into(),
                name: m["display_name"]
                    .as_str()
                    .or(m["displayName"].as_str())
                    .unwrap_or(id)
                    .into(),
                description: "Official API · prices and budget configured in Accounts".into(),
                efforts: vec![],
                default_effort: String::new(),
                is_default: true,
            });
        }
        let next = if *p == Provider::Googleapi {
            response["nextPageToken"]
                .as_str()
                .map(|t| ("pageToken", t.to_owned()))
        } else if response["has_more"] == true {
            response["last_id"]
                .as_str()
                .map(|t| ("after_id", t.to_owned()))
        } else {
            None
        };
        if let Some((name, token)) = next {
            if page == 19 {
                return Err("API model catalog pagination limit reached".into());
            }
            let mut u = reqwest::Url::parse(&format!("{}models", base(p))).unwrap();
            u.query_pairs_mut().append_pair(name, &token);
            url = u.into();
        } else {
            break;
        }
    }
    if rows.is_empty() {
        return Err("No API models available for this key".into());
    }
    Ok(rows)
}

pub(crate) struct Client<'a> {
    data: &'a Path,
    account: &'a Account,
    model: &'a str,
    http: reqwest::Client,
    secret: String,
    automatic: bool,
}

impl<'a> Client<'a> {
    pub(crate) fn new(
        data: &'a Path,
        account: &'a Account,
        model: &'a str,
        automatic: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            data,
            account,
            model,
            http: client()?,
            secret: key(&account.id)?,
            automatic,
        })
    }

    pub(crate) async fn step(
        &self,
        history: &[Value],
        tools: &[Value],
        max: Option<u32>,
        mut emit: impl FnMut(Activity),
    ) -> Result<(Value, crate::api_wire::Turn), String> {
        let c = config(self.data, &self.account.id)?;
        validate(&c)?;
        let max = max.unwrap_or(c.max_output_tokens).min(c.max_output_tokens);
        let (path, body) =
            crate::api_wire::request(&self.account.provider, self.model, history, tools, max)?;
        // Text-only inputs: bytes plus protocol/tool framing bound token counts conservatively.
        let bytes = serde_json::to_vec(&body).map_err(|_| "Invalid API request")?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("API request exceeds 8 MB; compact the conversation".into());
        }
        let bound = bytes.len() as u64 + 8192;
        let (request, c, reserved) = reserve(
            self.data,
            &self.account.id,
            self.model,
            bound,
            (c.max_output_tokens, max),
            self.automatic,
        )?;
        let activity = |value: i64, status: &str| {
            Activity{kind:"api-cost".into(),text:json!({"requestId":request,"model":self.model,"estimatedUsd":value as f64/1e6,"status":status,"source":"user-confirmed model rates"}).to_string()}
        };
        emit(activity(reserved, "reserved"));
        let url = format!("{}{}", base(&self.account.provider), path);
        let raw = auth(self.http.post(url), &self.account.provider, &self.secret)
            .header("content-type", "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(|_| "Official API request interrupted; its budget reservation is retained")?;
        if [400, 401, 403, 404, 422, 429].contains(&raw.status().as_u16()) {
            db(self.data)?
                .execute(
                    "UPDATE charges SET micros=0,status='rejected' WHERE id=?",
                    [&request],
                )
                .map_err(|_| "Cannot release rejected API reservation")?;
            emit(activity(0, "rejected"));
        }
        let response = json_response(raw).await?;
        let turn = crate::api_wire::response(&self.account.provider, &response)?;
        if let Some((i, o)) = turn.usage {
            let actual = cost(&c, i, o)?;
            db(self.data)?
                .execute(
                    "UPDATE charges SET micros=?,status='reported' WHERE id=?",
                    params![actual, request],
                )
                .map_err(|_| "Cannot record API usage")?;
            emit(activity(actual, "reported"));
        }
        Ok((response, turn))
    }
}

/// Price only opted-in, ready API accounts using their confirmed model rates.
pub(crate) fn rlm_price(root: &Path, account: &Account) -> Option<(String, f64)> {
    if !account.provider.is_api() || !eligible(root, account) {
        return None;
    }
    let config = config(root, &account.id).ok()?;
    Some((
        config.model,
        config.input_per_million + config.output_per_million,
    ))
}

pub async fn execute(
    input: Invocation<'_>,
    mut cancel: watch::Receiver<bool>,
    mut emit: impl FnMut(Activity) + Send,
) -> Result<String, String> {
    let data = root(input.config)?;
    let model = input.model.ok_or("Select an API model")?;
    let client = Client::new(data, input.account, model, input.automatic)?;
    let browser = input.browser.ok_or("Browser scope unavailable")?;
    use crate::{
        api_tools, code_mode,
        settings::{AppSettings, CodeMode},
    };
    let mut hub = tokio::sync::Mutex::new(crate::shared_tools::Hub::default());
    let prefs = AppSettings::load(data);
    let mode = prefs.code_mode;
    let effort = crate::rlm::Effort::for_turn(input.effort, prefs.rlm_effort);
    let endpoint = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let mut sandbox = None;
    let code_cancel = cancel.clone();
    let run = async {
        hub.get_mut().connect(&input, browser, &mut emit).await?;
        let mut prompt = input.prompt.to_owned();
        if mode == CodeMode::On {
            if let Some(sources) = input.rlm {
                prompt.push_str(&sources.hint());
            }
        }
        let mut history = crate::api_wire::initial(&input.account.provider, &prompt);
        let mut text = String::new();
        let mut call_ids = std::collections::HashSet::new();
        for _ in 0..32 {
            hub.get_mut()
                .refresh(input.access, browser, &mut emit)
                .await;
            let mut tools = crate::browser::tools().as_array().unwrap().clone();
            tools.extend(hub.get_mut().tools.clone());
            tools.extend(crate::api_tools::definitions(input.access));
            let mut aliases = std::collections::HashMap::new();
            for (index, tool) in tools.iter_mut().enumerate() {
                let name = tool["name"]
                    .as_str()
                    .ok_or("Shared tool has no name")?
                    .to_owned();
                if mode != CodeMode::On
                    && (name.len() > 64
                        || !name
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                {
                    let alias = format!("lumen_alias_{index}");
                    aliases.insert(alias.clone(), name);
                    tool["name"] = json!(alias);
                }
            }
            let advertised = if mode == CodeMode::On {
                vec![code_mode::definition_with_context(&tools, effort)]
            } else {
                tools.clone()
            };
            let (response, turn) = client.step(&history, &advertised, None, &mut emit).await?;
            if let Some((i, _)) = turn.usage {
                emit(Activity {
                    kind: "usage".into(),
                    text: i.to_string(),
                });
            }
            if !turn.text.is_empty() {
                text.push_str(&turn.text);
                emit(Activity {
                    kind: "replace".into(),
                    text: text.clone(),
                });
            }
            if let Some(error) = turn.error {
                return Err(error);
            }
            crate::api_wire::append_assistant(&input.account.provider, &mut history, &response);
            if turn.calls.is_empty() {
                if text.trim().is_empty() {
                    return Err("API returned no assistant text".into());
                }
                return Ok(text);
            }
            // Reject a repeated provider call ID before executing any action in that batch.
            for call in &turn.calls {
                if !call.id.is_empty() && !call_ids.insert(call.id.clone()) {
                    return Err("API repeated a tool call ID; no action from the repeated batch was executed".into());
                }
            }
            let mut results = vec![];
            for call in turn.calls {
                let name = aliases.get(&call.name).unwrap_or(&call.name);
                emit(Activity {
                    kind: "tool".into(),
                    text: name.clone(),
                });
                let result = if mode == CodeMode::On && name == code_mode::TOOL {
                    if let Some(code) = call.args["code"].as_str() {
                        if sandbox.is_none() {
                            sandbox = Some(code_mode::Session::new(code_cancel.clone())?.with_rlm(
                                crate::rlm::Turn::from_invocation(&input, data, &endpoint, effort),
                            ));
                        }
                        sandbox
                            .as_ref()
                            .unwrap()
                            .dispatch(&input, browser, &hub, &tools, code, &mut emit)
                            .await
                            .0
                    } else {
                        code_mode::result("Code Mode requires a string in code", true)
                    }
                } else if mode == CodeMode::On || name == code_mode::TOOL {
                    code_mode::result("Tool was not advertised for this turn", true)
                } else {
                    api_tools::dispatch(&input, browser, &hub, name, &call.args, &mut emit).await
                };
                results.push((call, result));
            }
            crate::api_wire::append_results(&input.account.provider, &mut history, results);
        }
        Err("API tool-step limit reached; continue explicitly".into())
    };
    let result = tokio::select! {biased;_=cancel.changed()=>Err("Cancelled".into()),r=run=>r};
    let cleanup = hub.get_mut().close().await;
    if result.is_ok() {
        cleanup?
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rlm_budget_reserves_only_the_bounded_output_and_requires_auto_consent() {
        let f = crate::test_support::Fixture::new();
        let mut config = Config {
            model: "test".into(),
            auto_enabled: false,
            budget_usd: 0.001,
            input_per_million: 1.,
            output_per_million: 10.,
            max_output_tokens: 4096,
            rates_confirmed: true,
        };
        let ledger = db(&f.root).unwrap();
        let save = |config: &Config| {
            ledger
                .execute(
                    "INSERT OR REPLACE INTO config VALUES('a',?)",
                    [serde_json::to_string(config).unwrap()],
                )
                .unwrap();
        };
        save(&config);
        assert!(reserve(&f.root, "a", "test", 10, (4096, 64), true).is_err());
        assert_eq!(spent(&ledger, "a").unwrap(), 0);
        config.auto_enabled = true;
        save(&config);
        let (_, _, bounded) = reserve(&f.root, "a", "test", 10, (4096, 64), true).unwrap();
        assert_eq!(bounded, 650);
        assert!(reserve(&f.root, "a", "test", 10, (4096, 64), true).is_err());
    }

    #[test]
    fn reservations_are_durable_and_pricing_bound() {
        let f = crate::test_support::Fixture::new();
        let c = Config {
            model: "test".into(),
            auto_enabled: true,
            budget_usd: 0.1,
            input_per_million: 1.,
            output_per_million: 10.,
            max_output_tokens: 4096,
            rates_confirmed: true,
        };
        db(&f.root)
            .unwrap()
            .execute(
                "INSERT INTO config VALUES('a',?)",
                [serde_json::to_string(&c).unwrap()],
            )
            .unwrap();
        assert!(reserve(&f.root, "a", "wrong", 10, (4096, 4096), false).is_err());
        assert!(reserve(&f.root, "a", "test", 10, (4096, 4096), false).is_ok());
        assert!(reserve(&f.root, "a", "test", 10, (4096, 4096), false).is_ok());
        assert!(reserve(&f.root, "a", "test", 10, (4096, 4096), false).is_err());
        assert!(spent(&db(&f.root).unwrap(), "a").unwrap() > 80_000);
        assert!(reserve(&f.root, "a", "test", 10, (256, 256), false)
            .unwrap_err()
            .contains("output limit changed"));
        assert!(money(f64::NAN).is_err());
        assert!(money(-1.).is_err());
    }
    #[test]
    fn concurrent_reservations_cannot_overspend() {
        let f = crate::test_support::Fixture::new();
        let c = Config {
            model: "test".into(),
            auto_enabled: true,
            budget_usd: 0.1,
            input_per_million: 1.,
            output_per_million: 10.,
            max_output_tokens: 4096,
            rates_confirmed: true,
        };
        db(&f.root)
            .unwrap()
            .execute(
                "INSERT INTO config VALUES('a',?)",
                [serde_json::to_string(&c).unwrap()],
            )
            .unwrap();
        let handles = (0..8)
            .map(|_| {
                let root = f.root.clone();
                std::thread::spawn(move || {
                    reserve(&root, "a", "test", 10, (4096, 4096), true).is_ok()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .filter(|ok| *ok)
                .count(),
            2
        );
        assert!(spent(&db(&f.root).unwrap(), "a").unwrap() <= 100_000);
    }
}
