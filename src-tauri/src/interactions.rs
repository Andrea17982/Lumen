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
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OptionItem {
    pub label: String,
    #[serde(default)]
    pub description: String,
}
pub type Answers = HashMap<String, Vec<String>>;
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
}
impl Default for Connection {
    fn default() -> Self {
        Self {
            owner: uuid::Uuid::new_v4().to_string(),
            completed: Default::default(),
        }
    }
}
impl Connection {
    pub fn ask(
        &self,
        questions: Vec<Question>,
        wire: String,
    ) -> Result<(Activity, oneshot::Receiver<Answers>), String> {
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
        let id = uuid::Uuid::new_v4().to_string();
        let (reply, receive) = oneshot::channel();
        let activity = Activity {
            kind: "interaction".into(),
            text: serde_json::json!({"id":id,"questions":questions}).to_string(),
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
#[tauri::command]
pub fn pending_questions() -> Result<Vec<String>, String> {
    Ok(pending()
        .lock()
        .map_err(|_| "Question lock failed")?
        .keys()
        .cloned()
        .collect())
}
#[tauri::command]
pub fn answer_question(id: String, answers: Answers) -> Result<(), String> {
    let mut requests = pending().lock().map_err(|_| "Question lock failed")?;
    let request = requests
        .get(&id)
        .ok_or("This question is no longer waiting for an answer")?;
    if answers.len() != request.questions.len()
        || request.questions.iter().any(|q| {
            answers.get(&q.id).is_none_or(|a| {
                a.is_empty()
                    || (!q.multi_select && a.len() != 1)
                    || a.iter().any(|s| s.trim().is_empty() || s.len() > 32_000)
            })
        })
    {
        return Err("Answer every question before sending".into());
    }
    let request = requests.remove(&id).unwrap();
    let public: Answers = answers
        .iter()
        .filter(|(id, _)| !request.questions.iter().any(|q| q.secret && &q.id == *id))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    request
        .reply
        .send(answers)
        .map_err(|_| "The CLI is no longer waiting".to_string())?;
    if let Ok(mut completed) = request.completed.lock() {
        completed.push(Activity {
            kind: "interaction-result".into(),
            text: serde_json::json!({"id":id,"answers":public}).to_string(),
        });
    }
    Ok(())
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
}
