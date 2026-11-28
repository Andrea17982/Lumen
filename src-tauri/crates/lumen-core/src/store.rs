use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::Mutex};

pub struct Store(Mutex<Connection>);
impl Store {
    pub(crate) fn queue_message(
        &self,
        session: &str,
        id: &str,
        text: &str,
        options: &str,
    ) -> Result<(), String> {
        let inserted = self.with(|c| c.execute("INSERT INTO queued_messages(id,session,text,options) SELECT ?1,?2,?3,?4 WHERE (SELECT count(*) FROM queued_messages WHERE session=?2)<20", params![id,session,text,options]))?;
        if inserted == 0 {
            return Err("This conversation already has 20 queued messages".into());
        }
        Ok(())
    }
    pub(crate) fn queued_messages(
        &self,
        session: &str,
    ) -> Result<Vec<crate::message_queue::Queued>, String> {
        self.with(|c| {
            let mut q = c.prepare(
                "SELECT id,text,claimed=1 FROM queued_messages WHERE session=?1 ORDER BY rowid",
            )?;
            let rows = q.query_map([session], |r| {
                Ok(crate::message_queue::Queued {
                    id: r.get(0)?,
                    text: r.get(1)?,
                    starting: r.get(2)?,
                })
            })?;
            rows.collect()
        })
    }
    pub(crate) fn take_queued_message(
        &self,
        session: &str,
        id: &str,
    ) -> Result<(String, String), String> {
        self.with(|c| c.query_row("UPDATE queued_messages SET claimed=2 WHERE session=?1 AND id=?2 AND claimed!=1 RETURNING text,options", params![session,id], |r|Ok((r.get(0)?,r.get(1)?))).optional())?.ok_or_else(|| "This message is already starting or has been removed".into())
    }
    pub(crate) fn claim_next_message(
        &self,
        session: &str,
    ) -> Result<Option<(String, String, String)>, String> {
        self.with(|c| c.query_row("UPDATE queued_messages SET claimed=1 WHERE rowid=(SELECT rowid FROM queued_messages WHERE session=?1 AND claimed=0 ORDER BY rowid LIMIT 1) RETURNING id,text,options", [session], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional())
    }
    pub(crate) fn release_queued_message(&self, id: &str) -> Result<(), String> {
        self.with(|c| c.execute("UPDATE queued_messages SET claimed=0 WHERE id=?1", [id]))
            .map(|_| ())
    }
    // T3 Code's insert-if-absent receipt pattern; source and MIT notice: command_receipts.rs.
    pub(crate) fn claim_command(
        &self,
        id: &str,
        fingerprint: &str,
    ) -> Result<Option<Option<String>>, String> {
        let (inserted, saved, result) = self.with(|c| {
            let inserted = c.execute("INSERT INTO command_receipts(id,fingerprint) VALUES(?1,?2) ON CONFLICT(id) DO NOTHING", params![id,fingerprint])?;
            let (saved, result): (String, Option<String>) = c.query_row("SELECT fingerprint,result FROM command_receipts WHERE id=?1", [id], |r| Ok((r.get(0)?,r.get(1)?)))?;
            Ok((inserted, saved, result))
        })?;
        if saved != fingerprint {
            return Err("Command ID was already used with different arguments".into());
        }
        Ok((inserted == 0).then_some(result))
    }
    pub(crate) fn complete_command(&self, id: &str, result: &str) -> Result<(), String> {
        self.with(|c| {
            c.execute(
                "UPDATE command_receipts SET result=?2 WHERE id=?1",
                params![id, result],
            )
        })
        .map(|_| ())
    }
    pub fn schedule_quota(&self, session: &str, message: &str) -> Result<(), String> {
        self.with(|c|c.execute("INSERT INTO quota_schedules VALUES(?1,?2) ON CONFLICT(session) DO UPDATE SET message=excluded.message",params![session,message])).map(|_|())
    }
    pub fn unschedule_quota(&self, session: &str) -> Result<(), String> {
        self.with(|c| c.execute("DELETE FROM quota_schedules WHERE session=?1", [session]))
            .map(|_| ())
    }
    pub fn quota_schedules(&self) -> Result<Vec<(String, String)>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT session,message FROM quota_schedules")?;
            let rows = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })
    }
    pub fn save_routine(&self, r: &crate::routines::Routine, options: &str) -> Result<(), String> {
        self.with(|c| {
            c.execute(
                "INSERT INTO routines(id,session,text,options,every_ms,next_at,until_at,anchor) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![r.id, r.session, r.text, options, r.every_ms, r.next_at, r.until_at, r.anchor],
            )
        })
        .map(|_| ())
    }
    pub fn reschedule_routine(&self, id: &str, next_at: i64) -> Result<(), String> {
        self.with(|c| {
            c.execute(
                "UPDATE routines SET next_at=?2 WHERE id=?1",
                params![id, next_at],
            )
        })
        .map(|_| ())
    }
    pub fn delete_routine(&self, id: &str) -> Result<(), String> {
        self.with(|c| c.execute("DELETE FROM routines WHERE id=?1", [id]))
            .map(|_| ())
    }
    pub fn routines(&self) -> Result<Vec<(crate::routines::Routine, String)>, String> {
        self.with(|c| {
            let mut q = c.prepare(
                "SELECT id,session,text,options,every_ms,next_at,until_at,anchor FROM routines ORDER BY next_at",
            )?;
            let rows = q.query_map([], |r| {
                Ok((
                    crate::routines::Routine {
                        id: r.get(0)?,
                        session: r.get(1)?,
                        text: r.get(2)?,
                        every_ms: r.get(4)?,
                        next_at: r.get(5)?,
                        until_at: r.get(6)?,
                        anchor: r.get(7)?,
                    },
                    r.get(3)?,
                ))
            })?;
            rows.collect()
        })
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut c = Connection::open(path).map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;
        crate::migrations::run(&mut c)?;
        c.execute(
            "UPDATE sessions SET status='interrupted' WHERE status IN ('running','waiting')",
            [],
        )
        .map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(c)))
    }
    pub(crate) fn with<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, String> {
        let guard = self.0.lock().map_err(|_| "Database lock failed")?;
        f(&guard).map_err(|e| e.to_string())
    }
    pub fn sessions(&self, search: &str, offset: u32) -> Result<Vec<Session>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT id,title,pinned,status,workspace,updated_at,workspace_id,access_mode FROM sessions WHERE instr(lower(title),lower(?1))>0 ORDER BY pinned DESC,sort_order DESC,updated_at DESC LIMIT 100 OFFSET ?2")?;
            let rows = q.query_map(params![search,offset], |r| Ok(Session{id:r.get(0)?,title:r.get(1)?,pinned:r.get(2)?,status:r.get(3)?,workspace:r.get(4)?,updated_at:r.get(5)?,workspace_id:r.get(6)?,access_mode: serde_json::from_str(&format!("\"{}\"", r.get::<_, String>(7)?)).unwrap_or_default()}))?;
            rows.collect()
        })
    }
    /// `PRAGMA integrity_check`: "ok", or what SQLite found damaged.
    pub fn integrity(&self) -> Result<String, String> {
        self.with(|c| {
            let mut q = c.prepare("PRAGMA integrity_check")?;
            let rows = q.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map(|r| r.join("; "))
        })
    }
    /// A consistent copy of the whole database, taken while Lumen runs.
    pub fn backup(&self, path: &Path) -> Result<(), String> {
        self.with(|c| {
            c.execute("VACUUM INTO ?1", [path.to_string_lossy()])
                .map(drop)
        })
    }
    /// Conversations whose status says a turn is running, waiting or queued.
    pub fn active_sessions(&self) -> Result<Vec<String>, String> {
        self.with(|c| {
            let mut q = c.prepare(
                "SELECT id FROM sessions WHERE status IN ('running','waiting','queued')",
            )?;
            let rows = q.query_map([], |r| r.get(0))?;
            rows.collect()
        })
    }
    pub fn session(&self, id: &str) -> Result<Session, String> {
        self.with(|c| {
            c.query_row(
                "SELECT id,title,pinned,status,workspace,updated_at,workspace_id,access_mode FROM sessions WHERE id=?",
                [id],
                |r| {
                    Ok(Session {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        pinned: r.get(2)?,
                        status: r.get(3)?,
                        workspace: r.get(4)?,
                        updated_at: r.get(5)?,
                        workspace_id: r.get(6)?,
                        access_mode: serde_json::from_str(&format!("\"{}\"", r.get::<_, String>(7)?)).unwrap_or_default(),
                    })
                },
            )
        })
    }
    pub fn sessions_updated_after(&self, at: i64) -> Result<Vec<(String, String, String)>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT id,title,workspace FROM sessions WHERE updated_at>?1")?;
            let rows = q.query_map([at], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect()
        })
    }
    pub fn message_created_at(&self, session: &str, message: &str) -> Result<i64, String> {
        self.with(|c| {
            c.query_row(
                "SELECT created_at FROM messages WHERE session_id=?1 AND id=?2",
                params![session, message],
                |r| r.get(0),
            )
        })
    }
    pub fn create(&self, workspace: String) -> Result<Session, String> {
        let s = Session {
            id: id(),
            title: "New conversation".into(),
            pinned: false,
            status: "idle".into(),
            workspace,
            updated_at: now(),
            workspace_id: None,
            access_mode: AccessMode::ReadOnly,
        };
        self.with(|c| {
            c.execute(
                "INSERT INTO sessions(id,title,pinned,status,workspace,updated_at,access_mode,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7,?6)",
                params![s.id, s.title, s.pinned, s.status, s.workspace, s.updated_at, "read-only"],
            )
        })?;
        Ok(s)
    }
    pub fn update(&self, s: &Session) -> Result<(), String> {
        self.with(|c| c.execute("UPDATE sessions SET title=?2,pinned=?3,status=?4,workspace=?5,updated_at=?6,access_mode=?7 WHERE id=?1",params![s.id,s.title,s.pinned,s.status,s.workspace,s.updated_at,serde_json::to_value(s.access_mode).unwrap().as_str().unwrap_or("read-only")]))?;
        Ok(())
    }
    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.with(|c| c.execute("DELETE FROM sessions WHERE id=?", [id]))?;
        Ok(())
    }
    pub fn status(&self, id: &str, status: &str) -> Result<(), String> {
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET status=?2,updated_at=?3 WHERE id=?1",
                params![id, status, now()],
            )
        })?;
        if matches!(status, "completed" | "partial" | "error" | "interrupted") {
            crate::workstate::refresh(self, id, None)?;
        }
        Ok(())
    }
    pub fn work_state(&self, id: &str) -> Result<Option<crate::workstate::WorkState>, String> {
        let text: Option<String> = self.with(|c| {
            c.query_row(
                "SELECT state FROM work_states WHERE session_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()
        })?;
        text.map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            .transpose()
    }
    pub fn work_seed(&self, id: &str) -> Result<Option<crate::workstate::WorkState>, String> {
        let text: Option<String> = self.with(|c| {
            c.query_row(
                "SELECT seed FROM work_states WHERE session_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map(Option::flatten)
        })?;
        text.map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            .transpose()
    }
    pub fn save_work_state(
        &self,
        id: &str,
        state: &crate::workstate::WorkState,
    ) -> Result<(), String> {
        let text = serde_json::to_string(state).map_err(|e| e.to_string())?;
        self.with(|c| c.execute("INSERT INTO work_states(session_id,state) VALUES(?1,?2) ON CONFLICT(session_id) DO UPDATE SET state=excluded.state", params![id,text]))?;
        Ok(())
    }
    pub fn seed_work_state(
        &self,
        id: &str,
        state: &crate::workstate::WorkState,
    ) -> Result<(), String> {
        let mut seed = state.clone();
        let session = self.session(id)?;
        seed.access = session.access_mode;
        if seed.workspace != session.workspace {
            seed.checks.clear();
            seed.check_evidence.clear();
            seed.pending
                .retain(|fact| !fact.text.starts_with("Validation failure remains:"));
        }
        seed.workspace = session.workspace;
        let text = serde_json::to_string(&seed).map_err(|e| e.to_string())?;
        self.with(|c| c.execute("INSERT INTO work_states(session_id,state,seed) VALUES(?1,?2,?2) ON CONFLICT(session_id) DO UPDATE SET state=excluded.state,seed=excluded.seed", params![id,text]))?;
        Ok(())
    }
    pub fn append_event(&self, payload: &mut serde_json::Value) -> Result<(), String> {
        // Broadcast/session status events have no message and already live in sessions.status.
        if payload["messageId"].as_str().unwrap_or("").is_empty() {
            return Ok(());
        }
        self.with(|c| {
            let session = payload["sessionId"].as_str().unwrap_or("");
            let message = payload["messageId"].as_str().unwrap_or("");
            c.execute(
                "INSERT INTO host_events(session,message,payload) VALUES(?1,?2,?3)",
                params![session, message, payload.to_string()],
            )?;
            payload["seq"] = serde_json::json!(c.last_insert_rowid());
            Ok(())
        })
    }
    pub fn message_snapshots(&self, session: &str) -> Result<Vec<serde_json::Value>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT m.id,m.role,m.text,m.activity,coalesce(s.seq,0) FROM messages m LEFT JOIN message_sequences s ON s.message=m.id WHERE m.session_id=? ORDER BY m.created_at,m.rowid")?;
            let rows = q.query_map([session], |r| {
                let activity: String = r.get(3)?;
                Ok(serde_json::json!({
                    "id":r.get::<_,String>(0)?, "role":r.get::<_,String>(1)?,
                    "text":r.get::<_,String>(2)?,
                    "activity":serde_json::from_str::<serde_json::Value>(&activity).unwrap_or(serde_json::json!([])),
                    "eventSeq":r.get::<_,i64>(4)?
                }))
            })?;
            let mut out = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            let mut q = c.prepare("SELECT seq,message,payload FROM host_events WHERE session=? ORDER BY seq")?;
            let events = q.query_map([session], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?;
            for row in events {
                let (seq, id, text) = row?;
                if id.is_empty(){continue;}
                let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                if !out.iter().any(|m|m["id"]==id) {
                    out.push(serde_json::json!({"id":id,"role":"assistant","text":"","activity":[],"eventSeq":0}));
                }
                let message = out.iter_mut().find(|m|m["id"]==id).unwrap();
                if seq <= message["eventSeq"].as_i64().unwrap_or(0) { continue; }
                replay_event(message,event["kind"].as_str().unwrap_or(""),event["text"].as_str().unwrap_or(""));
                message["eventSeq"]=serde_json::json!(seq);
            }
            let seq:i64=c.query_row("SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name='host_events'),0)",[],|r|r.get(0))?;
            for message in &mut out { message["snapshotSeq"]=serde_json::json!(seq); }
            Ok(out)
        })
    }
    pub fn messages(&self, session: &str) -> Result<Vec<Message>, String> {
        self.message_snapshots(session)?
            .into_iter()
            .map(|m| serde_json::from_value(m).map_err(|e| e.to_string()))
            .collect()
    }
    pub fn save_message(&self, s: &str, m: &Message) -> Result<(), String> {
        self.write_message(s, m, false)
    }
    /// Initial user submissions must not overwrite an existing message, even in another chat.
    pub fn insert_message(&self, s: &str, m: &Message) -> Result<(), String> {
        self.write_message(s, m, true)
    }
    fn write_message(&self, s: &str, m: &Message, insert_only: bool) -> Result<(), String> {
        self.with(|c| {
            let mut activities=m.activity.clone();
            let previous:Option<String>=c.query_row("SELECT activity FROM messages WHERE id=?1 AND session_id=?2",params![m.id,s],|r|r.get(0)).optional()?;
            if let Some(previous)=previous.and_then(|text|serde_json::from_str::<Vec<Activity>>(&text).ok()) {
                for activity in previous.into_iter().filter(|a|["interaction","interaction-result","child","image","browser"].contains(&a.kind.as_str())) {
                    if activity.kind == "browser" && !activities.iter().any(|a|a.kind == activity.kind && a.text == activity.text) {
                        activities.push(activity);
                        continue;
                    }
                    let id=serde_json::from_str::<serde_json::Value>(&activity.text).ok().and_then(|v|v["id"].as_str().map(str::to_owned));
                    if let Some(id)=id {
                        if !activities.iter().any(|a|a.kind==activity.kind && serde_json::from_str::<serde_json::Value>(&a.text).is_ok_and(|v|v["id"]==id)) { activities.push(activity); }
                    }
                }
            }
            let activity=serde_json::to_string(&activities).unwrap_or_default();
            let sql = if insert_only {
                "INSERT INTO messages VALUES(?1,?2,?3,?4,?5,?6)"
            } else {
                "INSERT INTO messages VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET text=excluded.text,activity=excluded.activity"
            };
            c.execute_batch("SAVEPOINT message_checkpoint")?;
            let result=(|| {
                let live=checkpoint_live(c,s,&m.id,&activities)?;
                c.execute(sql,params![m.id,s,m.role,m.text,activity,now()])?;
                if insert_only { c.execute("DELETE FROM queued_messages WHERE id=?1 AND session=?2",params![m.id,s])?; }
                c.execute("INSERT INTO message_sequences(message,seq) VALUES(?1,coalesce((SELECT max(seq) FROM host_events WHERE session=?2 AND message=?1),0)) ON CONFLICT(message) DO UPDATE SET seq=max(seq,excluded.seq)",params![m.id,s])?;
                c.execute("DELETE FROM host_events WHERE session=?1 AND message=?2 AND seq<=(SELECT seq FROM message_sequences WHERE message=?2)",params![s,m.id])?;
                for payload in live {
                    c.execute("INSERT INTO host_events(session,message,payload) VALUES(?1,?2,?3)",params![s,m.id,payload.to_string()])?;
                }
                Ok(())
            })();
            match result {Ok(())=>c.execute_batch("RELEASE message_checkpoint"),Err(e)=>{let _=c.execute_batch("ROLLBACK TO message_checkpoint; RELEASE message_checkpoint");Err(e)}}
        })?;
        Ok(())
    }
    pub fn record_external_activity(
        &self,
        session: &str,
        message: &str,
        activity: Activity,
    ) -> Result<(), String> {
        self.with(|c| {
            let current: String = c.query_row(
                "SELECT activity FROM messages WHERE session_id=?1 AND id=?2",
                params![session, message],
                |r| r.get(0),
            )?;
            let mut rows: Vec<Activity> = serde_json::from_str(&current).unwrap_or_default();
            crate::activity::record(&mut rows, activity);
            c.execute(
                "UPDATE messages SET activity=?1 WHERE session_id=?2 AND id=?3",
                params![
                    serde_json::to_string(&rows).unwrap_or_default(),
                    session,
                    message
                ],
            )
        })?;
        Ok(())
    }
    /// Every message that carries Orchestra state, with its session id.
    pub fn orchestra_messages(&self) -> Result<Vec<(String, Message)>, String> {
        let sessions:Vec<String>=self.with(|c|{
            let mut q=c.prepare(r#"SELECT DISTINCT session_id FROM messages WHERE activity LIKE '%"kind":"orchestra"%'"#)?;
            let rows=q.query_map([],|r|r.get(0))?;rows.collect()
        })?;
        let mut out = vec![];
        for session in sessions {
            for mut snapshot in self.message_snapshots(&session)? {
                let live = snapshot["live"].clone();
                let Some(activities) = snapshot["activity"].as_array_mut() else {
                    continue;
                };
                let Some(activity) = activities.iter_mut().find(|a| a["kind"] == "orchestra")
                else {
                    continue;
                };
                if let Ok(mut state) = serde_json::from_str::<serde_json::Value>(
                    activity["text"].as_str().unwrap_or(""),
                ) {
                    if let Some(tasks) = state["subtasks"].as_array_mut() {
                        for task in tasks {
                            if let Some(text) = task["id"]
                                .as_str()
                                .and_then(|id| live[id].as_str())
                                .filter(|s| !s.is_empty())
                            {
                                task["output"] = serde_json::json!(text);
                            }
                        }
                    }
                    activity["text"] = serde_json::json!(state.to_string());
                }
                out.push((
                    session.clone(),
                    serde_json::from_value(snapshot).map_err(|e| e.to_string())?,
                ));
            }
        }
        Ok(out)
    }
    /// Removes a message and everything after it (Rewind).
    pub fn truncate_from(&self, session: &str, message: &str) -> Result<(), String> {
        self.with(|c| {
            let tx = c.unchecked_transaction()?;
            let (at, row): (i64, i64) = tx.query_row(
                "SELECT created_at,rowid FROM messages WHERE id=?1 AND session_id=?2",
                params![message, session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            tx.execute(
                "DELETE FROM messages WHERE session_id=?1 AND (created_at>?2 OR (created_at=?2 AND rowid>=?3))",
                params![session, at, row],
            )?;
            // A remote CLI conversation still contains the discarded future. Its
            // IDs cannot safely resume even from an earlier retained response.
            // Clear only native resume markers, atomically with truncation.
            let retained = {
                let mut q = tx.prepare("SELECT id,activity FROM messages WHERE session_id=?1")?;
                let rows = q.query_map([session], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            for (id, raw) in retained {
                let Ok(mut activity) = serde_json::from_str::<Vec<Activity>>(&raw) else {
                    continue;
                };
                let before = activity.len();
                activity.retain(|a| a.kind != "native");
                if activity.len() != before {
                    tx.execute("UPDATE messages SET activity=?2 WHERE id=?1 AND session_id=?3",
                        params![id, serde_json::to_string(&activity).expect("serializable activities"), session])?;
                }
            }
            tx.commit()
        })?;
        crate::workstate::refresh(self, session, None)?;
        Ok(())
    }
    pub fn accounts(&self) -> Result<Vec<Account>, String> {
        self.with(|c| {
            let mut q = c.prepare(
                "SELECT id,name,provider FROM accounts WHERE provider != 'gemini' ORDER BY rowid",
            )?;
            let rows = q.query_map([], |r| {
                let p: String = r.get(2)?;
                Ok(Account {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    provider: Provider::from_id(&p).unwrap_or(Provider::Gpt),
                })
            })?;
            rows.collect()
        })
    }
    pub fn add_account(&self, name: String, provider: Provider) -> Result<Account, String> {
        let a = Account {
            id: id(),
            name,
            provider,
        };
        self.with(|c| {
            c.execute(
                "INSERT INTO accounts VALUES(?1,?2,?3)",
                params![a.id, a.name, a.provider.id()],
            )
        })?;
        Ok(a)
    }
    pub fn remove_account(&self, id: &str) -> Result<(), String> {
        self.with(|c| c.execute("DELETE FROM accounts WHERE id=?", [id]))?;
        Ok(())
    }
    pub(crate) fn claude_models(&self, id: &str) -> Result<Option<ClaudeModels>, String> {
        self.with(|c| {
            c.query_row(
                "SELECT fable,mythos FROM claude_models WHERE account=?1",
                [id],
                |r| {
                    Ok(ClaudeModels {
                        fable: r.get(0)?,
                        mythos: r.get(1)?,
                    })
                },
            )
            .optional()
        })
    }
    pub(crate) fn save_claude_models(&self, id: &str, models: ClaudeModels) -> Result<(), String> {
        self.with(|c| c.execute("INSERT INTO claude_models VALUES(?1,?2,?3) ON CONFLICT(account) DO UPDATE SET fable=excluded.fable,mythos=excluded.mythos", params![id,models.fable,models.mythos])).map(|_| ())
    }
    pub fn rename_account(&self, id: &str, name: &str) -> Result<(), String> {
        self.with(|c| c.execute("UPDATE accounts SET name=?2 WHERE id=?1", params![id, name]))?;
        Ok(())
    }
    pub fn workspaces(&self) -> Result<Vec<Workspace>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT id,name,folder FROM workspaces ORDER BY name")?;
            let rows = q.query_map([], |r| {
                Ok(Workspace {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    folder: r.get(2)?,
                })
            })?;
            rows.collect()
        })
    }
    pub fn add_workspace(&self, name: String, folder: String) -> Result<Workspace, String> {
        let w = Workspace {
            id: id(),
            name,
            folder,
        };
        self.with(|c| {
            c.execute(
                "INSERT INTO workspaces VALUES(?1,?2,?3)",
                params![w.id, w.name, w.folder],
            )
        })?;
        Ok(w)
    }
    pub fn rename_workspace(&self, id: &str, name: &str) -> Result<(), String> {
        self.with(|c| {
            c.execute(
                "UPDATE workspaces SET name=?2 WHERE id=?1",
                params![id, name],
            )
        })?;
        Ok(())
    }
    pub fn workspace_sessions(&self, id: &str) -> Result<Vec<String>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT id FROM sessions WHERE workspace_id=?1")?;
            let rows = q.query_map([id], |r| r.get(0))?;
            rows.collect()
        })
    }
    pub fn delete_workspace(&self, id: &str) -> Result<(), String> {
        self.with(|c| {
            let tx = c.unchecked_transaction()?;
            tx.execute("DELETE FROM sessions WHERE workspace_id=?1", [id])?;
            tx.execute("DELETE FROM workspaces WHERE id=?1", [id])?;
            tx.commit()
        })
    }
    pub fn bind_workspace(
        &self,
        id: &str,
        workspace_id: Option<&str>,
        folder: &str,
    ) -> Result<Session, String> {
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET workspace_id=?2,workspace=?3 WHERE id=?1",
                params![id, workspace_id, folder],
            )
        })?;
        self.session(id)
    }
    pub fn set_access_mode(&self, id: &str, mode: AccessMode) -> Result<Session, String> {
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET access_mode=?2 WHERE id=?1",
                params![
                    id,
                    serde_json::to_value(mode)
                        .unwrap()
                        .as_str()
                        .unwrap_or("read-only")
                ],
            )
        })?;
        self.session(id)
    }
    pub fn reorder(&self, id: &str, before_id: Option<&str>) -> Result<(), String> {
        self.with(|c| {
            let order: i64 = if let Some(before) = before_id {
                c.query_row(
                    "SELECT sort_order FROM sessions WHERE id=?1",
                    [before],
                    |r| r.get::<_, i64>(0),
                )? - 1
            } else {
                c.query_row(
                    "SELECT COALESCE(MAX(sort_order),0)+1 FROM sessions",
                    [],
                    |r| r.get(0),
                )?
            };
            c.execute(
                "UPDATE sessions SET sort_order=?2 WHERE id=?1",
                params![id, order],
            )
        })?;
        Ok(())
    }
}

/// Compact running-worker text separately from the orchestrator's own message checkpoint.
fn checkpoint_live(
    c: &Connection,
    session: &str,
    message: &str,
    activities: &[Activity],
) -> rusqlite::Result<Vec<serde_json::Value>> {
    use serde_json::{json, Value};
    let state = activities
        .iter()
        .find(|a| a.kind == "orchestra")
        .and_then(|a| serde_json::from_str::<Value>(&a.text).ok());
    let running: Vec<String> = state
        .as_ref()
        .and_then(|s| s["subtasks"].as_array())
        .into_iter()
        .flatten()
        .filter(|task| task["status"] == "running")
        .filter_map(|task| task["id"].as_str().map(str::to_owned))
        .collect();
    if running.is_empty() {
        return Ok(vec![]);
    }
    let mut live = json!({});
    let mut q=c.prepare("SELECT payload FROM host_events WHERE session=?1 AND message=?2 AND json_extract(payload,'$.kind')='orchestra-delta' ORDER BY seq")?;
    let rows = q.query_map(params![session, message], |r| r.get::<_, String>(0))?;
    for row in rows {
        if let Ok(event) = serde_json::from_str::<Value>(&row?) {
            replay_event(
                &mut live,
                "orchestra-delta",
                event["text"].as_str().unwrap_or(""),
            );
        }
    }
    Ok(running.into_iter().filter_map(|id|live["live"][&id].as_str().map(|text|json!({"sessionId":session,"messageId":message,"kind":"orchestra-delta","text":json!({"id":id,"kind":"replace","text":text}).to_string()}))).collect())
}

fn replay_event(message: &mut serde_json::Value, kind: &str, text: &str) {
    use serde_json::{json, Value};
    match kind {
        "delta" => {
            message["text"] = json!(format!(
                "{}{}",
                message["text"].as_str().unwrap_or(""),
                text
            ))
        }
        "replace" => message["text"] = json!(text),
        "start" | "done" | "status" | "workspace" => {}
        "orchestra-delta" => {
            let Ok(delta) = serde_json::from_str::<Value>(text) else {
                return;
            };
            let Some(id) = delta["id"].as_str() else {
                return;
            };
            let old = message["live"][id].as_str().unwrap_or("");
            let text = delta["text"].as_str().unwrap_or("");
            let next = if delta["kind"] == "replace" {
                text.to_string()
            } else {
                format!("{old}{text}")
            };
            if message["live"].is_null() {
                message["live"] = json!({});
            }
            message["live"][id] = json!(next
                .chars()
                .rev()
                .take(20000)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>());
        }
        _ => {
            let mut rows: Vec<Activity> =
                serde_json::from_value(message["activity"].clone()).unwrap_or_default();
            if kind == "orchestra" {
                rows.retain(|a| a.kind != "orchestra");
            }
            let already_saved = [
                "interaction",
                "interaction-result",
                "quota-scheduled",
                "browser",
                "image",
            ]
            .contains(&kind)
                && rows.iter().any(|a| a.kind == kind && a.text == text);
            if !already_saved {
                crate::activity::record(
                    &mut rows,
                    Activity {
                        kind: kind.into(),
                        text: text.into(),
                    },
                );
            }
            message["activity"] = serde_json::to_value(rows).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_and_cascade() {
        let db = Store::open(Path::new(":memory:")).unwrap();
        let s = db.create("/tmp".into()).unwrap();
        let m = Message {
            id: id(),
            role: "user".into(),
            text: "test".into(),
            activity: vec![],
        };
        db.save_message(&s.id, &m).unwrap();
        assert_eq!(db.messages(&s.id).unwrap().len(), 1);
        db.delete(&s.id).unwrap();
        assert!(db.messages(&s.id).unwrap().is_empty());
    }
    #[test]
    fn initial_submission_cannot_overwrite_an_existing_message() {
        let db = Store::open(Path::new(":memory:")).unwrap();
        let first = db.create("/tmp".into()).unwrap();
        let second = db.create("/tmp".into()).unwrap();
        let mut message = Message {
            id: id(),
            role: "user".into(),
            text: "Original request".into(),
            activity: vec![],
        };
        db.insert_message(&first.id, &message).unwrap();
        message.text = "Duplicate request".into();
        assert!(db.insert_message(&first.id, &message).is_err());
        assert!(db.insert_message(&second.id, &message).is_err());
        assert_eq!(db.messages(&first.id).unwrap()[0].text, "Original request");
        assert!(db.messages(&second.id).unwrap().is_empty());
    }
    #[test]
    fn upgrades_legacy_sessions_and_preserves_account_identity() {
        let path = std::env::temp_dir().join(format!("lumen-migration-{}.db", id()));
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,pinned INTEGER NOT NULL,status TEXT NOT NULL,workspace TEXT NOT NULL,updated_at INTEGER NOT NULL);INSERT INTO sessions VALUES('legacy','Keep me',1,'completed','old-folder',1);").unwrap();
        }
        {
            let db = Store::open(&path).unwrap();
            assert_eq!(db.session("legacy").unwrap().title, "Keep me");
            let w = db
                .add_workspace("Project".into(), "project-folder".into())
                .unwrap();
            let s = db
                .bind_workspace("legacy", Some(&w.id), "project/subfolder")
                .unwrap();
            assert_eq!(s.workspace_id.as_deref(), Some(w.id.as_str()));
            assert_eq!(s.workspace, "project/subfolder");
            let a = db.add_account("Before".into(), Provider::Gpt).unwrap();
            db.rename_account(&a.id, "After").unwrap();
            let updated = db.accounts().unwrap();
            assert_eq!(updated[0].id, a.id);
            assert_eq!(updated[0].name, "After");
        }
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod project_lifecycle_tests {
    use super::*;
    #[test]
    fn deleting_project_cascades_messages_but_keeps_moved_conversations() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let first = store
            .add_workspace("First".into(), "folder".into())
            .unwrap();
        let second = store
            .add_workspace("Second".into(), "folder".into())
            .unwrap();
        let deleted = store.create("folder".into()).unwrap();
        let kept = store.create("folder".into()).unwrap();
        for session in [&deleted, &kept] {
            store
                .bind_workspace(&session.id, Some(&first.id), "folder")
                .unwrap();
            store
                .insert_message(
                    &session.id,
                    &Message {
                        id: id(),
                        role: "user".into(),
                        text: "saved".into(),
                        activity: vec![],
                    },
                )
                .unwrap();
        }
        store.rename_workspace(&first.id, "Renamed").unwrap();
        assert!(store
            .workspaces()
            .unwrap()
            .iter()
            .any(|p| p.id == first.id && p.name == "Renamed"));
        store
            .bind_workspace(&kept.id, Some(&second.id), "folder")
            .unwrap();
        store.delete_workspace(&first.id).unwrap();
        assert!(store.session(&deleted.id).is_err());
        assert!(store.messages(&deleted.id).unwrap().is_empty());
        assert_eq!(store.messages(&kept.id).unwrap().len(), 1);
        assert_eq!(
            store.session(&kept.id).unwrap().workspace_id.as_deref(),
            Some(second.id.as_str())
        );
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;
    #[test]
    fn journal_recovers_uncheckpointed_text_without_replaying_saved_events() {
        let f = crate::test_support::Fixture::new();
        let store = Store::open(&f.root.join("lumen.db")).unwrap();
        let session = store.create(f.root.to_string_lossy().into()).unwrap();
        let m = Message {
            id: id(),
            role: "assistant".into(),
            text: "saved ".into(),
            activity: vec![],
        };
        store.save_message(&session.id, &m).unwrap();
        let event = |kind: &str, text: &str| serde_json::json!({"sessionId":session.id,"messageId":m.id,"kind":kind,"text":text});
        let mut e = event("delta", "partial");
        let mut session_event = serde_json::json!({"sessionId":session.id,"messageId":"","kind":"done","text":"completed"});
        store.append_event(&mut session_event).unwrap();
        assert_eq!(store.messages(&session.id).unwrap().len(), 1);
        store.append_event(&mut e).unwrap();
        assert!(e["seq"].as_i64().unwrap() > 0);
        let rows = store.message_snapshots(&session.id).unwrap();
        assert_eq!(rows[0]["text"], "saved partial");
        assert_eq!(rows[0]["eventSeq"], e["seq"]);
        let recovered = store.messages(&session.id).unwrap().remove(0);
        store.save_message(&session.id, &recovered).unwrap();
        assert_eq!(
            store.messages(&session.id).unwrap()[0].text,
            "saved partial"
        );
        let mut e = event("delta", " again");
        store.append_event(&mut e).unwrap();
        assert_eq!(
            store.messages(&session.id).unwrap()[0].text,
            "saved partial again"
        );
        assert_eq!(
            store
                .with(
                    |c| c.query_row("SELECT count(*) FROM host_events", [], |r| r
                        .get::<_, i64>(0))
                )
                .unwrap(),
            1
        );
        let reopen = Store::open(&f.root.join("lumen.db")).unwrap();
        assert_eq!(
            reopen.messages(&session.id).unwrap()[0].text,
            "saved partial again"
        );
    }
    #[tokio::test]
    async fn worker_output_survives_other_worker_checkpoints_and_crash_recovery() {
        let f = crate::test_support::Fixture::new();
        let session = f
            .core
            .store
            .create(f.root.to_string_lossy().into())
            .unwrap();
        let state = serde_json::json!({"stage":"running","workspace":session.workspace,"subtasks":[{"id":"first","status":"running"},{"id":"second","status":"running"}]});
        let mut m = Message {
            id: id(),
            role: "assistant".into(),
            text: "".into(),
            activity: vec![Activity {
                kind: "orchestra".into(),
                text: state.to_string(),
            }],
        };
        f.core.store.save_message(&session.id, &m).unwrap();
        let mut event = serde_json::json!({"sessionId":session.id,"messageId":m.id,"kind":"orchestra-delta","text":serde_json::json!({"id":"second","kind":"delta","text":"partial worker result"}).to_string()});
        f.core.store.append_event(&mut event).unwrap();
        let mut state = state;
        state["subtasks"][0]["status"] = serde_json::json!("done");
        m.activity[0].text = state.to_string();
        f.core.store.save_message(&session.id, &m).unwrap();
        f.core.store.save_message(&session.id, &m).unwrap();
        let rows = f.core.store.message_snapshots(&session.id).unwrap();
        assert_eq!(rows[0]["live"]["second"], "partial worker result");
        assert_eq!(
            f.core
                .store
                .with(
                    |c| c.query_row("SELECT count(*) FROM host_events", [], |r| r
                        .get::<_, i64>(0))
                )
                .unwrap(),
            1
        );
        crate::orchestra::recover(&f.core);
        let recovered = f.core.store.messages(&session.id).unwrap();
        let state: serde_json::Value =
            serde_json::from_str(&recovered[0].activity[0].text).unwrap();
        assert_eq!(state["subtasks"][1]["status"], "interrupted");
        assert!(state["subtasks"][1]["output"]
            .as_str()
            .unwrap()
            .contains("partial worker result"));
        assert_eq!(
            f.core
                .store
                .with(
                    |c| c.query_row("SELECT count(*) FROM host_events", [], |r| r
                        .get::<_, i64>(0))
                )
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn command_boundary_rejects_missing_typed_arguments_and_unknown_commands() {
        let f = crate::test_support::Fixture::new();
        let app = crate::host::EventHub::default();
        assert!(crate::dispatch(
            app.clone(),
            &f.core,
            "create_session",
            serde_json::json!({})
        )
        .await
        .unwrap_err()
        .contains("workspace"));
        assert!(
            crate::dispatch(app, &f.core, "not_a_command", serde_json::json!({}))
                .await
                .is_err()
        );
    }
}
