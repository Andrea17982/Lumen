use crate::models::*;
use rusqlite::{params, Connection};
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
          UPDATE sessions SET status='interrupted' WHERE status IN ('running','waiting');")
          .map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(c)))
    }
    fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
        let guard = self.0.lock().map_err(|_| "Database lock failed")?;
        f(&guard).map_err(|e| e.to_string())
    }
    pub fn sessions(&self, search: &str, offset: u32) -> Result<Vec<Session>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT id,title,pinned,status,workspace,updated_at FROM sessions WHERE instr(lower(title),lower(?1))>0 ORDER BY pinned DESC,updated_at DESC LIMIT 100 OFFSET ?2")?;
            let rows = q.query_map(params![search,offset], |r| Ok(Session{id:r.get(0)?,title:r.get(1)?,pinned:r.get(2)?,status:r.get(3)?,workspace:r.get(4)?,updated_at:r.get(5)?}))?;
            rows.collect()
        })
    }
    pub fn session(&self, id: &str) -> Result<Session, String> {
        self.with(|c| {
            c.query_row(
                "SELECT id,title,pinned,status,workspace,updated_at FROM sessions WHERE id=?",
                [id],
                |r| {
                    Ok(Session {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        pinned: r.get(2)?,
                        status: r.get(3)?,
                        workspace: r.get(4)?,
                        updated_at: r.get(5)?,
                    })
                },
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
        };
        self.with(|c| {
            c.execute(
                "INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6)",
                params![s.id, s.title, s.pinned, s.status, s.workspace, s.updated_at],
            )
        })?;
        Ok(s)
    }
    pub fn update(&self, s: &Session) -> Result<(), String> {
        self.with(|c| c.execute("UPDATE sessions SET title=?2,pinned=?3,status=?4,workspace=?5,updated_at=?6 WHERE id=?1",params![s.id,s.title,s.pinned,s.status,s.workspace,s.updated_at]))?;
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
    pub fn accounts(&self) -> Result<Vec<Account>, String> {
        self.with(|c| {
            let mut q = c.prepare("SELECT id,name,provider FROM accounts ORDER BY rowid")?;
            let rows = q.query_map([], |r| {
                let p: String = r.get(2)?;
                Ok(Account {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    provider: if p == "claude" {
                        Provider::Claude
                    } else {
                        Provider::Gpt
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
        let p = if a.provider == Provider::Claude {
            "claude"
        } else {
            "gpt"
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
}
