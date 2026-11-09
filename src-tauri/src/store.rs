use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::Mutex};

pub struct Store(Mutex<Connection>);
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let c = Connection::open(path).map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,pinned INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL,workspace TEXT NOT NULL,updated_at INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,role TEXT NOT NULL,text TEXT NOT NULL,activity TEXT NOT NULL,created_at INTEGER NOT NULL);
          CREATE INDEX IF NOT EXISTS messages_session ON messages(session_id,created_at);
          CREATE TABLE IF NOT EXISTS accounts(id TEXT PRIMARY KEY,name TEXT NOT NULL,provider TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS workspaces(id TEXT PRIMARY KEY,name TEXT NOT NULL,folder TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS work_states(session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,state TEXT NOT NULL,seed TEXT);
          UPDATE sessions SET status='interrupted' WHERE status IN ('running','waiting');")
          .map_err(|e| e.to_string())?;
        let columns = {
            let mut q = c
                .prepare("PRAGMA table_info(sessions)")
                .map_err(|e| e.to_string())?;
            let names = q
                .query_map([], |r| r.get::<_, String>(1))
                .map_err(|e| e.to_string())?;
            names
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };
        if !columns.iter().any(|n| n == "workspace_id") {
            c.execute(
                "ALTER TABLE sessions ADD COLUMN workspace_id TEXT REFERENCES workspaces(id)",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        if !columns.iter().any(|n| n == "access_mode") {
            c.execute(
                "ALTER TABLE sessions ADD COLUMN access_mode TEXT NOT NULL DEFAULT 'read-only'",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        if !columns.iter().any(|n| n == "sort_order") {
            c.execute(
                "ALTER TABLE sessions ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(|e| e.to_string())?;
            c.execute("UPDATE sessions SET sort_order=updated_at", [])
                .map_err(|e| e.to_string())?;
        }
        Ok(Self(Mutex::new(c)))
    }
    fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
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
        if matches!(status, "completed" | "error" | "interrupted") {
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
        seed.workspace = session.workspace;
        let text = serde_json::to_string(&seed).map_err(|e| e.to_string())?;
        self.with(|c| c.execute("INSERT INTO work_states(session_id,state,seed) VALUES(?1,?2,?2) ON CONFLICT(session_id) DO UPDATE SET state=excluded.state,seed=excluded.seed", params![id,text]))?;
        Ok(())
    }
    pub fn messages(&self, session: &str) -> Result<Vec<Message>, String> {
        self.with(|c| {
            let mut q=c.prepare("SELECT id,role,text,activity FROM messages WHERE session_id=? ORDER BY created_at,rowid")?;
            let rows=q.query_map([session],|r| { let a:String=r.get(3)?; Ok(Message{id:r.get(0)?,role:r.get(1)?,text:r.get(2)?,activity:serde_json::from_str(&a).unwrap_or_default()}) })?;
            rows.collect()
        })
    }
    pub fn save_message(&self, s: &str, m: &Message) -> Result<(), String> {
        let activity = serde_json::to_string(&m.activity).map_err(|e| e.to_string())?;
        self.with(|c| c.execute("INSERT INTO messages VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET text=excluded.text,activity=excluded.activity",params![m.id,s,m.role,m.text,activity,now()]))?;
        Ok(())
    }
    /// Every message that carries Orchestra state, with its session id.
    pub fn orchestra_messages(&self) -> Result<Vec<(String, Message)>, String> {
        self.with(|c| {
            let mut q = c.prepare(r#"SELECT session_id,id,role,text,activity FROM messages WHERE activity LIKE '%"kind":"orchestra"%'"#)?;
            let rows = q.query_map([], |r| {
                let a: String = r.get(4)?;
                Ok((
                    r.get(0)?,
                    Message {
                        id: r.get(1)?,
                        role: r.get(2)?,
                        text: r.get(3)?,
                        activity: serde_json::from_str(&a).unwrap_or_default(),
                    },
                ))
            })?;
            rows.collect()
        })
    }
    /// Removes a message and everything after it (Rewind).
    pub fn truncate_from(&self, session: &str, message: &str) -> Result<(), String> {
        self.with(|c| {
            let (at, row): (i64, i64) = c.query_row(
                "SELECT created_at,rowid FROM messages WHERE id=?1 AND session_id=?2",
                params![message, session],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            c.execute(
                "DELETE FROM messages WHERE session_id=?1 AND (created_at>?2 OR (created_at=?2 AND rowid>=?3))",
                params![session, at, row],
            )
        })?;
        crate::workstate::refresh(self, session, None)?;
        Ok(())
    }
    pub fn accounts(&self) -> Result<Vec<Account>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT id,name,provider FROM accounts ORDER BY rowid")?;
            let rows = q.query_map([], |r| {
                let p: String = r.get(2)?;
                Ok(Account {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    provider: match p.as_str() {
                        "claude" => Provider::Claude,
                        "gpt" => Provider::Gpt,
                        "gemini" => Provider::Gemini,
                        "antigravity" => Provider::Antigravity,
                        "ollama" => Provider::Ollama,
                        _ => Provider::Gpt,
                    },
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
        let p = match &a.provider {
            Provider::Claude => "claude",
            Provider::Gpt => "gpt",
            Provider::Gemini => "gemini",
            Provider::Antigravity => "antigravity",
            Provider::Ollama => "ollama",
        };
        self.with(|c| {
            c.execute(
                "INSERT INTO accounts VALUES(?1,?2,?3)",
                params![a.id, a.name, p],
            )
        })?;
        Ok(a)
    }
    pub fn remove_account(&self, id: &str) -> Result<(), String> {
        self.with(|c| c.execute("DELETE FROM accounts WHERE id=?", [id]))?;
        Ok(())
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
