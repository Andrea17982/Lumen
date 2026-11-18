//! Paired local benchmark. CI uses a scripted loopback model; live inference is opt-in.
use crate::{
    api_tools,
    code_mode::{self, Session},
    models::{AccessMode, Account, Provider},
    ollama_agent::{endpoint, local_model},
    providers::Invocation,
    test_support::Fixture,
};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const FILES: usize = 20;
const WINDOW: u64 = 12_500_000;
const READ: &str = "lumen_read_file";
const WRITE: &str = "lumen_write_file";
const SCAN: &str = "bench_scan_log";
const CHECKS: [&str; 3] = ["bench_test", "bench_lint", "bench_build"];
const REQUESTS: [(&str, &str, &str); 3] = [
    ("A", "Change the value alpha to beta in every file item-00.txt through item-19.txt. Preserve the other line in each file. Make the edits with the available tools.", "edited"),
    ("B", "Scan shards 0, 1, 2 and 3 of the log using the available tools. Sum the units from the PAID entries only and answer with that integer alone.", "28"),
    ("C", "Use the three tools to run the project's test, lint and build checks. Answer only with the name of the check that failed, without the bench_ prefix.", "build"),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'a> {
    tier: &'a str,
    suite: &'a str,
    model: &'a str,
    effort: &'a str,
    seed: u32,
    arm: &'a str,
    native_tools: bool,
    input_tokens: usize,
    output_tokens: usize,
    reasoning_tokens_est: Option<usize>,
    ttft_ms: Option<u64>,
    token_source: &'a str,
    max_request_bytes: usize,
    max_input_tokens: usize,
    rounds: usize,
    host_calls: usize,
    tool_errors: usize,
    script_errors: usize,
    code_fallback: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter_trace: Option<Value>,
    script_ms: u64,
    wall_ms: u64,
    success: bool,
    partial: f64,
    cost_usd: u64,
    error: Option<String>,
}

struct Model<'a> {
    http: reqwest::Client,
    base: reqwest::Url,
    id: &'a str,
    effort: &'a str,
    native: bool,
    offline: bool,
}

async fn response(model: &Model<'_>, wire: &str) -> Result<(Value, Option<u64>), String> {
    let started = Instant::now();
    let mut stream = model
        .http
        .post(model.base.join("api/chat").unwrap())
        .body(wire.to_owned())
        .header("content-type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes_stream();
    let mut buffer = Vec::new();
    let mut text = String::new();
    let mut thinking = String::new();
    let mut calls = Vec::new();
    let mut ttft = None;
    while let Some(chunk) = stream.next().await {
        buffer.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        if buffer.len() > 2 * 1024 * 1024 {
            return Err("Ollama stream frame exceeds 2 MiB".into());
        }
        while let Some(newline) = buffer.iter().position(|&b| b == b'\n') {
            let line = buffer.drain(..=newline).collect::<Vec<_>>();
            let mut frame: Value = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
            local_model(model.id, &frame)?;
            if let Some(error) = frame["error"].as_str() {
                return Err(error.into());
            }
            let delta = frame["message"]["content"].as_str().unwrap_or_default();
            let reasoning = frame["message"]["thinking"].as_str().unwrap_or_default();
            if !model.offline && (!delta.is_empty() || !reasoning.is_empty()) {
                ttft.get_or_insert_with(|| started.elapsed().as_millis() as u64);
            }
            text.push_str(delta);
            thinking.push_str(reasoning);
            if let Some(batch) = frame["message"]["tool_calls"].as_array() {
                calls.extend(batch.iter().cloned());
            }
            if frame["done"] == true {
                frame["message"] = json!({"role":"assistant","content":text,"thinking":thinking,"tool_calls":calls});
                return Ok((frame, ttft));
            }
        }
    }
    Err("Ollama stream ended before completion".into())
}

fn definition(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required}})
}

fn definitions(suite: &str) -> Vec<Value> {
    match suite {
        "A" => api_tools::definitions(AccessMode::FileEdits)
            .into_iter()
            .filter(|d| d["name"] == READ || d["name"] == WRITE)
            .collect(),
        "B" => vec![definition(
            SCAN,
            "Read a bounded log window from one of four shards. Returns log text.",
            json!({"shard":{"type":"integer"}}),
            &["shard"],
        )],
        _ => CHECKS
            .iter()
            .map(|name| {
                definition(
                    name,
                    &format!("Run the {} check.", &name[6..]),
                    json!({}),
                    &[],
                )
            })
            .collect(),
    }
}

fn prepare(root: &Path, suite: &str) {
    if suite == "A" {
        for i in 0..FILES {
            fs::write(
                root.join(format!("item-{i:02}.txt")),
                format!("value=alpha\nkeep={i:02}\n"),
            )
            .unwrap();
        }
    }
    if suite == "B" {
        let mut file = File::create(root.join("fixture.log")).unwrap();
        file.set_len(4 * WINDOW).unwrap();
        for shard in 0..4 {
            file.seek(SeekFrom::Start(shard * WINDOW)).unwrap();
            for i in 0..30 {
                writeln!(file, "INFO event={shard:02}{i:02} state=WAIT units=99").unwrap();
            }
            writeln!(
                file,
                "INFO event={shard:02}30 state=PAID units={}",
                4 + shard * 2
            )
            .unwrap();
        }
    }
    if suite == "C" {
        for check in CHECKS {
            fs::write(
                root.join(format!("{check}.cjs")),
                format!(
                    "console.log('check {}'); process.exitCode = {};\n",
                    &check[6..],
                    u8::from(check == "bench_build")
                ),
            )
            .unwrap();
        }
    }
}

async fn dispatch(input: &Invocation<'_>, suite: &str, name: &str, args: &Value) -> Value {
    match (suite, name) {
        ("A", READ | WRITE) => {
            let path = args["path"].as_str().unwrap_or_default();
            if !(0..FILES).any(|i| path == format!("item-{i:02}.txt")) {
                return code_mode::result(
                    "Use a fixture filename item-00.txt through item-19.txt",
                    true,
                );
            }
            api_tools::call(input, name, args, &mut |_| {}).await
        }
        ("B", SCAN) => {
            let Some(shard) = args["shard"].as_u64().filter(|&n| n < 4) else {
                return code_mode::result("Choose an integer shard from 0 to 3", true);
            };
            let read = || -> std::io::Result<String> {
                let mut file = File::open(input.workspace.join("fixture.log"))?;
                file.seek(SeekFrom::Start(shard * WINDOW))?;
                let mut window = String::new();
                file.take(2048).read_to_string(&mut window)?;
                window.truncate(window.find('\0').unwrap_or(window.len()));
                Ok(window)
            };
            match read() {
                Ok(text) => code_mode::result(&text, false),
                Err(error) => code_mode::result(&error.to_string(), true),
            }
        }
        ("C", name) if CHECKS.contains(&name) => {
            // Model output can select a fixture check, but never supplies command text.
            let command = format!("node {name}.cjs");
            api_tools::call(
                input,
                "lumen_run_command",
                &json!({"command":command}),
                &mut |_| {},
            )
            .await
        }
        _ => code_mode::result("Tool is outside this benchmark task", true),
    }
}

fn function_tool(definition: &Value) -> Value {
    json!({"type":"function","function":{"name":definition["name"],"description":definition["description"],"parameters":definition["inputSchema"]}})
}

async fn trial<'a>(
    model: &'a Model<'a>,
    case: (&'a str, &'a str, &'a str),
    cm: bool,
    seed: u32,
) -> Row<'a> {
    let (suite, prompt, expected) = case;
    let fixture = Fixture::new();
    let workspace = fixture.directory("project");
    let config = fixture.directory("accounts/bench");
    prepare(&workspace, suite);
    let account = Account {
        id: "bench".into(),
        name: "bench".into(),
        provider: Provider::Ollama,
    };
    let input = Invocation {
        automatic: false,
        account: &account,
        config: &config,
        workspace: &workspace,
        prompt,
        model: Some(model.id),
        effort: model.effort,
        access: AccessMode::Autopilot,
        approvals: None,
        resume: None,
        browser_scope: None,
        browser_artifacts: None,
        browser: None,
    };
    let (_cancel, stop) = watch::channel(false);
    let mut sandbox = None;
    let definitions = definitions(suite);
    let classic_tools = definitions.iter().map(function_tool).collect::<Vec<_>>();
    let code_tools = vec![function_tool(&code_mode::definition(&definitions))];
    let mut mode = code_mode::OllamaMode::new(cm);
    let mut filter_trace = Vec::new();
    let mut messages = vec![json!({"role":"user","content":prompt})];
    if !model.native {
        messages.insert(0, json!({"role":"system","content":""}));
    }
    let started = Instant::now();
    let mut row = Row {
        tier: if model.offline { "T0" } else { "T1" },
        suite,
        model: model.id,
        effort: model.effort,
        seed,
        arm: if cm { "CM" } else { "TC" },
        native_tools: model.native,
        input_tokens: 0,
        output_tokens: 0,
        reasoning_tokens_est: None,
        ttft_ms: None,
        token_source: if model.offline {
            "tiktoken-o200k-wire-estimate"
        } else {
            "ollama"
        },
        max_request_bytes: 0,
        max_input_tokens: 0,
        rounds: 0,
        host_calls: 0,
        tool_errors: 0,
        script_errors: 0,
        code_fallback: false,
        filter_trace: None,
        script_ms: 0,
        wall_ms: 0,
        success: false,
        partial: 0.,
        cost_usd: 0,
        error: None,
    };
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let errors = std::sync::atomic::AtomicUsize::new(0);
    let covered = std::sync::atomic::AtomicUsize::new(0);
    let invoke = |name: String, args: Value| {
        let (input, calls, errors, covered) = (&input, &calls, &errors, &covered);
        async move {
            use std::sync::atomic::Ordering::Relaxed;
            calls.fetch_add(1, Relaxed);
            let result = dispatch(input, suite, &name, &args).await;
            let expected_failure = suite == "C"
                && name == "bench_build"
                && result["content"][0]["text"]
                    .as_str()
                    .is_some_and(|s| s.contains("check build"));
            if result["isError"] == true && !expected_failure {
                errors.fetch_add(1, Relaxed);
            } else if suite == "B" && name == SCAN {
                covered.fetch_or(1 << args["shard"].as_u64().unwrap(), Relaxed);
            } else if suite == "C" {
                if let Some(index) = CHECKS.iter().position(|&check| check == name) {
                    covered.fetch_or(1 << index, Relaxed);
                }
            }
            result
        }
    };
    let outcome: Result<String, String> = async {
        // The same bounded benchmark budget applies to both arms.
        for _ in 0..12 {
            let code = mode.active;
            let tools = if code { &code_tools } else { &classic_tools };
            if !model.native {
                let action = if code { "Return action=code and code=<script>." }
                    else { "Return action=tool, tool=<name>, arguments=<object>, and text empty." };
                messages[0]["content"] = json!(format!(
                    "{action} When finished, return action=final and your answer in text. Tools: {}", json!(tools)
                ));
            }
            let mut request = json!({"model":model.id,"messages":messages,"stream":true,
                "keep_alive":"10m","options":{"temperature":0,"seed":seed,"num_ctx":8192,"num_predict":768}});
            if model.effort != "default" {
                request["think"] = json!(model.effort);
            }
            if model.native {
                request["tools"] = json!(tools);
            } else {
                request["format"] = crate::ollama_agent::schema(tools, code);
            }
            let wire = request.to_string();
            row.max_request_bytes = row.max_request_bytes.max(wire.len());
            row.rounds += 1;
            let (response, ttft) = response(model, &wire).await?;
            if row.rounds == 1 {
                row.ttft_ms = ttft;
            }
            let (input_tokens, output_tokens) = if model.offline {
                let bpe = tiktoken_rs::o200k_base_singleton();
                (bpe.encode_ordinary(&wire).len(), bpe.encode_ordinary(&response["message"].to_string()).len())
            } else {
                (response["prompt_eval_count"].as_u64().ok_or("missing Ollama input usage")? as usize,
                 response["eval_count"].as_u64().ok_or("missing Ollama output usage")? as usize)
            };
            row.input_tokens += input_tokens;
            row.output_tokens += output_tokens;
            row.max_input_tokens = row.max_input_tokens.max(input_tokens);
            let message = &response["message"];
            if let Some(thinking) = message["thinking"].as_str().filter(|s| !s.is_empty()) {
                let count = tiktoken_rs::o200k_base_singleton().encode_ordinary(thinking).len();
                *row.reasoning_tokens_est.get_or_insert(0) += count;
            }
            let text = message["content"].as_str().unwrap_or_default();
            let requested = if model.native {
                message["tool_calls"].as_array().cloned().unwrap_or_default()
            } else {
                let action: Value = serde_json::from_str(text).map_err(|_| "invalid structured Ollama action")?;
                if action["action"] == "final" {
                    return Ok(action["text"].as_str().unwrap_or_default().into());
                }
                vec![match action["action"].as_str() {
                    Some("code") if code => native_call(code_mode::TOOL, json!({"code":action["code"]})),
                    Some("tool") if !code => native_call(action["tool"].as_str().unwrap_or_default(), action["arguments"].clone()),
                    _ => return Err("unknown structured Ollama action".into()),
                }]
            };
            if requested.is_empty() { return Ok(text.into()); }
            if requested.len() > 16 { return Err("Ollama native per-step tool cap exceeded".into()); }
            if model.native {
                messages.push(json!({"role":"assistant","content":text,"tool_calls":requested}));
            } else {
                messages.push(json!({"role":"assistant","content":text}));
            }
            for call in requested {
                let name = call["function"]["name"].as_str().unwrap_or_default();
                let args = &call["function"]["arguments"];
                let result = if code && mode.active && name == code_mode::TOOL {
                    let before = Instant::now();
                    if sandbox.is_none() {
                        sandbox = Some(Session::new(stop.clone())?);
                    }
                    let output = sandbox.as_ref().unwrap()
                        .run(args["code"].as_str().ok_or("missing code")?, &definitions, &invoke)
                        .await;
                    row.script_ms += before.elapsed().as_millis() as u64;
                    row.script_errors += usize::from(output.is_err());
                    let syntax = matches!(output, Err(code_mode::ScriptError::Syntax(_)));
                    let mut result = code_mode::report(output, &mut |_| {});
                    if mode.observe(syntax, &mut result).is_some() {
                        sandbox = None;
                        row.code_fallback = true;
                    }
                    if suite == "B" && !model.offline {
                        filter_trace.push(json!({"code": args["code"], "result": result["content"][0]["text"]}));
                    }
                    result
                } else if !code && name != code_mode::TOOL {
                    invoke(name.into(), args.clone()).await
                } else {
                    row.tool_errors += 1;
                    code_mode::result("Tool is outside the advertised arm", true)
                };
                messages.push(json!({"role":if model.native {"tool"} else {"user"},
                    "tool_name": name,"content":result.to_string()}));
            }
        }
        Err("benchmark step cap reached".into())
    }.await;
    row.host_calls = calls.into_inner();
    row.tool_errors += errors.into_inner();
    row.wall_ms = started.elapsed().as_millis() as u64;
    if suite == "A" {
        row.partial = (0..FILES)
            .filter(|&i| {
                fs::read_to_string(workspace.join(format!("item-{i:02}.txt"))).ok()
                    == Some(format!("value=beta\nkeep={i:02}\n"))
            })
            .count() as f64
            / FILES as f64;
    } else {
        let mask = if suite == "B" { 0b1111 } else { 0b111 };
        let correct = outcome.as_ref().is_ok_and(|text| text.trim() == expected);
        row.partial = f64::from(correct && covered.into_inner() == mask);
    }
    row.success = row.partial == 1. && outcome.is_ok();
    if suite == "B" && !model.offline {
        row.filter_trace = Some(json!({"scripts": filter_trace, "answer": outcome.as_ref().ok()}));
    }
    row.error = outcome.err();
    row
}

fn native_call(name: &str, args: Value) -> Value {
    json!({"function":{"name":name,"arguments":args}})
}

async fn replay(axum::Json(request): axum::Json<Value>) -> String {
    let messages = request["messages"].as_array().unwrap();
    let content = messages[0]["content"].as_str().unwrap();
    let case = REQUESTS
        .iter()
        .find(|&&(_, prompt, _)| prompt == content)
        .unwrap();
    let (suite, _, _) = *case;
    let round = messages.iter().filter(|m| m["role"] == "assistant").count();
    let cm = request["tools"][0]["function"]["name"] == code_mode::TOOL;
    let calls = if cm && round == 0 {
        let script = match suite {
            "A" => {
                r#"(async () => {
                for (let i = 0; i < 20; i++) {
                    const path = 'item-' + String(i).padStart(2, '0') + '.txt';
                    const file = await tools.lumen_read_file({path});
                    await tools.lumen_write_file({path, content: file.content[0].text.replace('alpha', 'beta')});
                }
                return 'edited';
            })()"#
            }
            "B" => {
                r#"(async () => {
                const logs = await Promise.all([0,1,2,3].map(shard => tools.bench_scan_log({shard})));
                return logs.reduce((sum, row) => sum + Number(row.content[0].text.match(/state=PAID units=(\d+)/)[1]), 0);
            })()"#
            }
            _ => {
                r#"(async () => {
                const names = ['test', 'lint', 'build'];
                const results = await Promise.all(names.map(name => tools['bench_' + name]()));
                return names[results.findIndex(result => result.isError)];
            })()"#
            }
        };
        vec![native_call(code_mode::TOOL, json!({"code":script}))]
    } else if !cm {
        match (suite, round) {
            ("A", 0..=3) => {
                let start = if round % 2 == 0 { 0 } else { 16 };
                (start..(start + 16).min(FILES))
                    .map(|i| {
                        let path = format!("item-{i:02}.txt");
                        if round < 2 {
                            native_call(READ, json!({"path":path}))
                        } else {
                            let text = messages
                                .iter()
                                .filter(|m| m["role"] == "tool")
                                .nth(i)
                                .and_then(|m| result_text(m).map(|(_, text)| text))
                                .unwrap_or_default()
                                .replace("alpha", "beta");
                            native_call(WRITE, json!({"path":path,"content":text}))
                        }
                    })
                    .collect()
            }
            ("B", 0) => (0..4)
                .map(|i| native_call(SCAN, json!({"shard":i})))
                .collect(),
            ("C", 0) => CHECKS
                .iter()
                .map(|name| native_call(name, json!({})))
                .collect(),
            _ => vec![],
        }
    } else {
        vec![]
    };
    let message = if calls.is_empty() {
        let results = messages
            .iter()
            .filter(|m| m["role"] == "tool")
            .filter_map(|m| {
                result_text(m)
                    .map(|(error, text)| (error, text, m["tool_name"].as_str().unwrap_or("")))
            })
            .collect::<Vec<_>>();
        let answer = if cm {
            results
                .last()
                .map(|(_, text, _)| text.clone())
                .unwrap_or_default()
        } else {
            match suite {
                "B" => results
                    .iter()
                    .filter_map(|(_, text, _)| {
                        text.lines().find_map(|line| {
                            line.split_once("state=PAID units=")?.1.parse::<u32>().ok()
                        })
                    })
                    .sum::<u32>()
                    .to_string(),
                "C" => results
                    .iter()
                    .find(|(error, _, _)| *error)
                    .map(|(_, _, name)| name.trim_start_matches("bench_").to_owned())
                    .unwrap_or_default(),
                _ => "edited".into(),
            }
        };
        json!({"role":"assistant","content":answer})
    } else {
        json!({"role":"assistant","content":"","tool_calls":calls})
    };
    format!("{}\n", json!({"message":message,"done":true}))
}

fn result_text(message: &Value) -> Option<(bool, String)> {
    let result: Value = serde_json::from_str(message["content"].as_str()?).ok()?;
    Some((
        result["isError"] == true,
        result["content"][0]["text"].as_str()?.into(),
    ))
}

fn save(row: &Row<'_>) {
    let line = serde_json::to_string(row).unwrap();
    println!("{line}");
    if let Some(path) = std::env::var_os("LUMEN_BENCH_OUTPUT") {
        let mut file = File::options()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap()
}

#[tokio::test]
async fn bench_t0() {
    let address = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = endpoint(&address.local_addr().unwrap().to_string()).unwrap();
    let app = axum::Router::new().route("/api/chat", axum::routing::post(replay));
    let server = tokio::spawn(async move { axum::serve(address, app).await.unwrap() });
    let model = Model {
        http: client(),
        base,
        id: "scripted",
        effort: "default",
        native: true,
        offline: true,
    };
    for case in REQUESTS {
        let tc = trial(&model, case, false, 0).await;
        let cm = trial(&model, case, true, 0).await;
        save(&tc);
        save(&cm);
        assert!(tc.success && cm.success);
        assert_eq!(tc.host_calls, cm.host_calls);
        let (classic, code) = match case.0 {
            "A" => (5, 2),
            _ => (2, 2),
        };
        assert_eq!((tc.rounds, cm.rounds), (classic, code));
        if case.0 == "B" {
            assert!(cm.max_request_bytes * 2 < tc.max_request_bytes);
        }
    }
    server.abort();
}

#[tokio::test]
#[ignore = "uses installed local Ollama models; no cloud endpoints"]
async fn bench_t1() {
    let base = endpoint(&std::env::var("LUMEN_BENCH_URL").expect("set LUMEN_BENCH_URL")).unwrap();
    let ids = std::env::var("LUMEN_BENCH_MODELS").expect("set LUMEN_BENCH_MODELS");
    let suite = std::env::var("LUMEN_BENCH_SUITE").ok();
    assert!(suite
        .as_deref()
        .is_none_or(|suite| REQUESTS.iter().any(|case| case.0 == suite)));
    for id in ids.split(',') {
        let http = client();
        local_model(id, &Value::Null).unwrap();
        let show: Value = http
            .post(base.join("api/show").unwrap())
            .json(&json!({"model":id}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        local_model(id, &show).unwrap();
        let supported = |name: &str| {
            show["capabilities"]
                .as_array()
                .is_some_and(|list| list.iter().any(|capability| capability == name))
        };
        let effort = std::env::var("LUMEN_BENCH_EFFORT").unwrap_or_else(|_| "default".into());
        assert!(matches!(
            effort.as_str(),
            "default" | "low" | "medium" | "high"
        ));
        assert!(effort == "default" || supported("thinking"));
        let model = Model {
            http,
            base: base.clone(),
            id,
            effort: &effort,
            native: supported("tools"),
            offline: false,
        };
        let seeds = std::env::var("LUMEN_BENCH_SEEDS")
            .unwrap_or_else(|_| "5".into())
            .parse::<u32>()
            .unwrap();
        for seed in 0..seeds {
            for case in REQUESTS {
                if suite.as_deref().is_some_and(|suite| suite != case.0) {
                    continue;
                }
                // Interleave paired arms in reproducible pseudorandom order.
                let first =
                    (seed ^ (case.0.as_bytes()[0] as u32)).wrapping_mul(0x45d9f3b) >> 29 & 1 == 0;
                for cm in [first, !first] {
                    save(&trial(&model, case, cm, seed).await);
                }
            }
        }
    }
}
