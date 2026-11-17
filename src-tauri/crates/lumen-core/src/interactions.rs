//! Native model questions are input, never permission grants. Requests live only as long as
//! the CLI connection that owns them; a stale transcript cannot answer a later turn.
use crate::models::Activity;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::oneshot;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub options: Vec<OptionItem>,
    #[serde(default)]
    pub multi_select: bool,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub optional: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OptionItem {
    pub label: String,
    #[serde(default)]
    pub description: String,
}
pub type Answers = HashMap<String, Vec<String>>;

tokio::task_local! {
    /// Set for a turn whose run options select Auto: plain clarifying questions are
    /// answered by Lumen instead of the user. Permission and connector requests never are.
    pub static AUTO_ANSWER: bool;
}
/// Free-text reply Lumen gives in Auto when no option exists.
pub const ASSUMPTION: &str = "The user is not available (Lumen Auto). Proceed with the most reasonable assumption consistent with LUMEN.md, the code and the task; state the assumption in your answer. Ask the user only before a destructive or external action.";
fn validate(questions: &[Question]) -> Result<(), String> {
    if questions.is_empty()
        || questions.len() > 32
        || questions
            .iter()
            .any(|q| q.id.is_empty() || q.question.is_empty())
        || questions
            .iter()
            .map(|q| &q.id)
            .collect::<std::collections::HashSet<_>>()
            .len()
            != questions.len()
    {
        return Err("The CLI sent an invalid question".into());
    }
    Ok(())
}
fn safe(text: &str) -> bool {
    crate::autonomy::command(text).0 == crate::autonomy::Class::Reversible
}
/// Deterministic Auto answers: the recommended (or first) safe option, or [`ASSUMPTION`].
/// `None` escalates to the user: secret input, or a question/option whose text names a
/// risky or external action.
pub fn auto_answers(questions: &[Question]) -> Option<(Answers, Vec<(String, String)>)> {
    let mut answers = Answers::new();
    let mut made = vec![];
    for q in questions {
        if q.secret || !safe(&q.question) {
            return None;
        }
        let answer = if q.options.is_empty() {
            ASSUMPTION.to_string()
        } else {
            let options: Vec<_> = q
                .options
                .iter()
                .filter(|o| safe(&format!("{} {}", o.label, o.description)))
                .collect();
            options
                .iter()
                .find(|o| o.label.to_lowercase().contains("recommended"))
                .or(options.first())?
                .label
                .clone()
        };
        made.push((q.question.clone(), answer.clone()));
        answers.insert(q.id.clone(), vec![answer]);
    }
    Some((answers, made))
}
/// The "Assumptions Lumen made" section for a finished Auto reply, if any.
pub fn assumptions_section(activity: &[Activity]) -> Option<String> {
    let items: Vec<String> = activity
        .iter()
        .filter(|a| a.kind == "assumption")
        .filter_map(|a| serde_json::from_str::<serde_json::Value>(&a.text).ok())
        .flat_map(|v| v["items"].as_array().cloned().unwrap_or_default())
        .map(|i| {
            let answer = i["answer"].as_str().unwrap_or_default();
            let answer = if answer == ASSUMPTION {
                "no answer given; the agent chose a reasonable default"
            } else {
                answer
            };
            format!(
                "- {} → {}",
                i["question"].as_str().unwrap_or_default(),
                answer
            )
        })
        .collect();
    (!items.is_empty()).then(|| {
        format!(
            "

### Assumptions Lumen made
{}

Reply to correct any of them.",
            items.join(
                "
"
            )
        )
    })
}
pub fn answer_context(activity: &[Activity]) -> Vec<String> {
    let mut facts = vec![];
    for result in activity
        .iter()
        .filter(|a| a.kind == "interaction-result")
        .filter_map(|a| serde_json::from_str::<serde_json::Value>(&a.text).ok())
    {
        if result["cancelled"] == true {
            continue;
        }
        let request = activity
            .iter()
            .filter(|a| a.kind == "interaction")
            .filter_map(|a| serde_json::from_str::<serde_json::Value>(&a.text).ok())
            .find(|v| v["id"] == result["id"]);
        if result["kind"] == "permission"
            || result["kind"] == "connector"
            || request
                .as_ref()
                .is_some_and(|r| r["kind"] == "connector" || r["kind"] == "permission")
        {
            continue;
        }
        if let Some(answers) = result["answers"].as_object() {
            for (id, values) in answers {
                let question = request
                    .as_ref()
                    .and_then(|r| r["questions"].as_array())
                    .and_then(|qs| qs.iter().find(|q| q["id"] == *id))
                    .and_then(|q| q["question"].as_str())
                    .unwrap_or(id);
                let answers = values
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                if !answers.is_empty() {
                    facts.push(format!("User clarification (permissions are controlled separately): {question} → {answers}"));
                }
            }
        }
    }
    facts
}
struct Pending {
    connector: Option<crate::connectors::Request>,
    permission: bool,
    owner: String,
    wire: String,
    questions: Vec<Question>,
    reply: oneshot::Sender<Answers>,
    completed: Arc<Mutex<Vec<Activity>>>,
}
fn pending() -> &'static Mutex<HashMap<String, Pending>> {
    static PENDING: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}
pub struct Connection {
    owner: String,
    completed: Arc<Mutex<Vec<Activity>>>,
    auto: bool,
}
impl Default for Connection {
    fn default() -> Self {
        Self {
            owner: uuid::Uuid::new_v4().to_string(),
            completed: Default::default(),
            auto: AUTO_ANSWER.try_with(|a| *a).unwrap_or(false),
        }
    }
}
impl Connection {
    pub fn ask(
        &self,
        questions: Vec<Question>,
        wire: String,
    ) -> Result<(Activity, oneshot::Receiver<Answers>), String> {
        validate(&questions)?;
        if self.auto {
            if let Some((answers, made)) = auto_answers(&questions) {
                let (reply, receive) = oneshot::channel();
                let _ = reply.send(answers);
                let items: Vec<_> = made
                    .into_iter()
                    .map(|(question, answer)| serde_json::json!({"question":question,"answer":answer}))
                    .collect();
                return Ok((
                    Activity {
                        kind: "assumption".into(),
                        text: serde_json::json!({ "items": items }).to_string(),
                    },
                    receive,
                ));
            }
        }
        self.ask_inner(questions, wire, None)
    }
    /// Always waits for the user, even in Auto (consent-style questions).
    pub fn ask_user(
        &self,
        questions: Vec<Question>,
        wire: String,
    ) -> Result<(Activity, oneshot::Receiver<Answers>), String> {
        self.ask_inner(questions, wire, None)
    }
    pub fn ask_permission(
        &self,
        questions: Vec<Question>,
        wire: String,
    ) -> Result<(Activity, oneshot::Receiver<Answers>), String> {
        let (mut activity, receive) = self.ask_inner(questions, wire, None)?;
        let mut value: serde_json::Value =
            serde_json::from_str(&activity.text).map_err(|_| "Invalid permission request")?;
        if let Some(request) = pending()
            .lock()
            .map_err(|_| "Question lock failed")?
            .get_mut(value["id"].as_str().ok_or("Invalid permission request")?)
        {
            request.permission = true;
        }
        value["kind"] = "permission".into();
        activity.text = value.to_string();
        Ok((activity, receive))
    }
    pub fn ask_connector(
        &self,
        request: crate::connectors::Request,
        wire: String,
    ) -> Result<(Activity, oneshot::Receiver<Answers>), String> {
        self.ask_inner(request.questions.clone(), wire, Some(request))
    }
    fn ask_inner(
        &self,
        questions: Vec<Question>,
        wire: String,
        connector: Option<crate::connectors::Request>,
    ) -> Result<(Activity, oneshot::Receiver<Answers>), String> {
        validate(&questions)?;
        let id = uuid::Uuid::new_v4().to_string();
        let (reply, receive) = oneshot::channel();
        let mut description = serde_json::json!({"id":id,"questions":questions});
        if let Some(request) = &connector {
            description["kind"] = "connector".into();
            description["actionId"] = "__lumen_connector_action__".into();
            description["urlOrigin"] = serde_json::json!(request.origin);
        }
        let activity = Activity {
            kind: "interaction".into(),
            text: description.to_string(),
        };
        if activity.text.len() > 64_000 {
            return Err("The CLI question exceeds the 64 KB input limit".into());
        }
        pending()
            .lock()
            .map_err(|_| "Question lock failed")?
            .insert(
                id,
                Pending {
                    permission: false,
                    connector,
                    owner: self.owner.clone(),
                    wire,
                    questions,
                    reply,
                    completed: self.completed.clone(),
                },
            );
        Ok((activity, receive))
    }
    pub fn cancel(&self, wire: &str) {
        if let Ok(mut requests) = pending().lock() {
            requests.retain(|id, p| {
                if p.owner == self.owner && p.wire == wire {
                    if let Ok(mut completed) = p.completed.lock() {
                        completed.push(Activity {
                            kind: "interaction-result".into(),
                            text: serde_json::json!({"id":id,"cancelled":true,"answers":{}})
                                .to_string(),
                        });
                    }
                    false
                } else {
                    true
                }
            });
        }
    }
    pub fn drain(&self, emit: &mut impl FnMut(Activity)) {
        if let Ok(mut completed) = self.completed.lock() {
            for activity in completed.drain(..) {
                emit(activity);
            }
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        if let Ok(mut requests) = pending().lock() {
            requests.retain(|_, p| p.owner != self.owner);
        }
    }
}
#[crate::command]
pub fn pending_questions() -> Result<Vec<String>, String> {
    Ok(pending()
        .lock()
        .map_err(|_| "Question lock failed")?
        .keys()
        .cloned()
        .collect())
}
#[crate::command]
pub fn answer_question(id: String, answers: Answers) -> Result<(), String> {
    let mut requests = pending().lock().map_err(|_| "Question lock failed")?;
    let request = requests
        .get(&id)
        .ok_or("This question is no longer waiting for an answer")?;
    let connector_reply = request
        .connector
        .as_ref()
        .map(|request| request.response(&answers))
        .transpose()?;
    let declined = connector_reply
        .as_ref()
        .is_some_and(|reply| reply["action"] != "accept");
    if answers.len() != request.questions.len()
        || request.questions.iter().any(|q| {
            answers.get(&q.id).is_none_or(|a| {
                (!declined && !q.optional && a.is_empty())
                    || (!q.multi_select && a.len() > 1)
                    || a.iter().any(|s| s.trim().is_empty() || s.len() > 32_000)
            })
        })
    {
        return Err("Answer every question before sending".into());
    }
    let request = requests.remove(&id).unwrap();
    let public: Answers = answers
        .iter()
        .filter(|(id, _)| {
            request.connector.is_none()
                && !request.questions.iter().any(|q| q.secret && &q.id == *id)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    request
        .reply
        .send(answers)
        .map_err(|_| "The CLI is no longer waiting".to_string())?;
    if let Ok(mut completed) = request.completed.lock() {
        completed.push(Activity {
            kind: "interaction-result".into(),
            text: serde_json::json!({"id":id,"answers":public,"kind":if request.permission {"permission"} else if request.connector.is_some(){"connector"}else{"question"}}).to_string(),
        });
    }
    Ok(())
}

pub fn connector_url(id: &str) -> Result<String, String> {
    pending()
        .lock()
        .map_err(|_| "Question lock failed")?
        .get(id)
        .and_then(|request| request.connector.as_ref())
        .and_then(|request| request.url.clone())
        .ok_or_else(|| "This connector sign-in is no longer waiting".into())
}

#[crate::command]
pub fn open_connector_url(id: String) -> Result<String, String> {
    connector_url(&id)
}

pub fn codex_questions(params: &serde_json::Value) -> Result<Vec<Question>, String> {
    let mut raw = params["questions"].clone();
    if let Some(qs) = raw.as_array_mut() {
        for q in qs {
            if q["options"].is_null() {
                q["options"] = serde_json::json!([]);
            }
        }
    }
    let mut questions: Vec<Question> = serde_json::from_value(raw).map_err(|e| e.to_string())?;
    // The official schema permits a null options array.
    for q in &mut questions {
        q.secret = params["questions"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["id"] == q.id))
            .is_some_and(|v| v["isSecret"] == true);
    }
    Ok(questions)
}
pub fn claude_questions(input: &serde_json::Value) -> Result<Vec<Question>, String> {
    let questions = input["questions"]
        .as_array()
        .ok_or("Missing Claude questions")?;
    questions
        .iter()
        .map(|q| {
            let question = q["question"]
                .as_str()
                .ok_or("Missing question")?
                .to_string();
            Ok(Question {
                id: question.clone(),
                question,
                header: q["header"].as_str().unwrap_or("").into(),
                options: serde_json::from_value(q["options"].clone()).map_err(|e| e.to_string())?,
                multi_select: q["multiSelect"] == true,
                secret: false,
                optional: false,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn questions_are_validated_once_and_expire_with_the_connection() {
        let c = Connection::default();
        let qs = vec![Question {
            id: "q".into(),
            question: "Choose".into(),
            header: "Choice".into(),
            options: vec![],
            multi_select: false,
            secret: false,
            optional: false,
        }];
        let (a, rx) = c.ask(qs.clone(), "first".into()).unwrap();
        let id = serde_json::from_str::<serde_json::Value>(&a.text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(answer_question(id.clone(), HashMap::new()).is_err());
        let answer = HashMap::from([("q".into(), vec!["Custom".into()])]);
        answer_question(id.clone(), answer.clone()).unwrap();
        assert_eq!(rx.await.unwrap(), answer);
        assert!(answer_question(id, answer).is_err());
        let (_, rx) = c.ask(qs.clone(), "second".into()).unwrap();
        c.cancel("second");
        assert!(rx.await.is_err());
        let (_, rx) = c.ask(qs, "third".into()).unwrap();
        drop(c);
        assert!(rx.await.is_err());
    }
    #[tokio::test]
    async fn one_shot_permission_answers_never_become_task_requirements() {
        let c = Connection::default();
        let (request, rx) = c
            .ask_permission(
                vec![Question {
                    id: "action".into(),
                    header: "Action".into(),
                    question: "Write file?".into(),
                    options: vec![],
                    multi_select: false,
                    secret: false,
                    optional: false,
                }],
                "permission".into(),
            )
            .unwrap();
        let id = serde_json::from_str::<serde_json::Value>(&request.text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        answer_question(
            id,
            HashMap::from([("action".into(), vec!["Allow once".into()])]),
        )
        .unwrap();
        rx.await.unwrap();
        let mut activity = vec![request];
        c.drain(&mut |a| activity.push(a));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&activity[1].text).unwrap()["kind"],
            "permission"
        );
        assert!(answer_context(&activity).is_empty());
    }

    fn q(id: &str, text: &str, options: &[&str]) -> Question {
        Question {
            id: id.into(),
            header: "H".into(),
            question: text.into(),
            options: options
                .iter()
                .map(|l| OptionItem {
                    label: (*l).into(),
                    description: String::new(),
                })
                .collect(),
            multi_select: false,
            secret: false,
            optional: false,
        }
    }

    #[test]
    fn auto_answers_are_deterministic_and_conservative() {
        let (answers, made) = auto_answers(&[
            q("a", "Which style?", &["Tabs", "Spaces (Recommended)"]),
            q("b", "Which runner?", &["Vitest", "Jest"]),
            q("c", "Anything else?", &[]),
        ])
        .unwrap();
        assert_eq!(answers["a"], ["Spaces (Recommended)"]);
        assert_eq!(answers["b"], ["Vitest"]);
        assert_eq!(answers["c"], [ASSUMPTION]);
        assert_eq!(made.len(), 3);
        // A risky option is never chosen; a risky question or a secret goes to the user.
        let (answers, _) =
            auto_answers(&[q("a", "Clean up?", &["git reset --hard", "Keep changes"])]).unwrap();
        assert_eq!(answers["a"], ["Keep changes"]);
        assert!(auto_answers(&[q("a", "Run rm -rf build?", &["Yes", "No"])]).is_none());
        assert!(auto_answers(&[q("a", "Push?", &["git push --force"])]).is_none());
        let mut secret = q("a", "Token?", &[]);
        secret.secret = true;
        assert!(auto_answers(&[secret]).is_none());
    }

    #[tokio::test]
    async fn auto_mode_answers_questions_but_never_permissions() {
        let questions = vec![q(
            "switch",
            "Continue with another model?",
            &["Continue", "Stop"],
        )];
        // Outside Auto the question waits for the user, unchanged.
        let c = Connection::default();
        let (a, _rx) = c.ask(questions.clone(), "manual".into()).unwrap();
        assert_eq!(a.kind, "interaction");
        AUTO_ANSWER
            .scope(true, async {
                let c = Connection::default();
                let (a, rx) = c.ask(questions.clone(), "auto".into()).unwrap();
                assert_eq!(a.kind, "assumption");
                assert_eq!(rx.await.unwrap()["switch"], ["Continue"]);
                let section = assumptions_section(&[a]).unwrap();
                assert!(section.contains("Assumptions Lumen made"));
                assert!(section.contains("Continue with another model? → Continue"));
                let (a, _rx) = c.ask_permission(questions.clone(), "perm".into()).unwrap();
                assert_eq!(a.kind, "interaction");
                assert!(a.text.contains("\"permission\""));
                let (a, _rx) = c.ask_user(questions.clone(), "consent".into()).unwrap();
                assert_eq!(a.kind, "interaction");
                assert!(c.ask(vec![], "bad".into()).is_err());
            })
            .await;
    }
}
