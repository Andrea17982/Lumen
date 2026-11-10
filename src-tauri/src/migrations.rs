//! Transactional, versioned SQLite schema upgrades. A newer database is never downgraded.
use rusqlite::Connection;
const CURRENT: u32 = 3;
fn column(c: &Connection, table: &str, name: &str) -> rusqlite::Result<bool> {
    let mut q = c.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = q.query_map([], |r| r.get::<_, String>(1))?;
    for row in rows {
        if row? == name {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn run(c: &mut Connection) -> Result<(), String> {
    let version: u32 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version > CURRENT {
        return Err(format!("This database uses schema {version}; update Lumen before opening it (supported: {CURRENT})"));
    }
    let tx = c.transaction().map_err(|e| e.to_string())?;
    let apply = || -> rusqlite::Result<()> {
        tx.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY,applied_at INTEGER NOT NULL);")?;
        for next in (version + 1)..=CURRENT {
            match next {
                1 => tx.execute_batch("CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,pinned INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL,workspace TEXT NOT NULL,updated_at INTEGER NOT NULL);
                    CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,role TEXT NOT NULL,text TEXT NOT NULL,activity TEXT NOT NULL,created_at INTEGER NOT NULL);
                    CREATE INDEX IF NOT EXISTS messages_session ON messages(session_id,created_at);
                    CREATE TABLE IF NOT EXISTS accounts(id TEXT PRIMARY KEY,name TEXT NOT NULL,provider TEXT NOT NULL);
                    CREATE TABLE IF NOT EXISTS workspaces(id TEXT PRIMARY KEY,name TEXT NOT NULL,folder TEXT NOT NULL);")?,
                2 => {
                    for (name,sql) in [
                        ("workspace_id","ALTER TABLE sessions ADD COLUMN workspace_id TEXT REFERENCES workspaces(id)"),
                        ("access_mode","ALTER TABLE sessions ADD COLUMN access_mode TEXT NOT NULL DEFAULT 'read-only'"),
                        ("sort_order","ALTER TABLE sessions ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0"),
                    ] {
                        if !column(&tx,"sessions",name)? {
                            tx.execute_batch(sql)?;
                            if name == "sort_order" { tx.execute("UPDATE sessions SET sort_order=updated_at",[])?; }
                        }
                    }
                },
                3 => {
                    tx.execute_batch("CREATE TABLE IF NOT EXISTS work_states(session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,state TEXT NOT NULL,seed TEXT);")?;
                    if !column(&tx,"work_states","seed")? { tx.execute_batch("ALTER TABLE work_states ADD COLUMN seed TEXT")?; }
                },
                _ => unreachable!(),
            }
            tx.execute(
                "INSERT INTO schema_migrations(version,applied_at) VALUES(?1,unixepoch())",
                [next],
            )?;
            tx.pragma_update(None, "user_version", next)?;
        }
        Ok(())
    };
    apply()
        .map_err(|e| format!("Database upgrade failed; no schema changes were committed: {e}"))?;
    tx.commit().map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrades_are_idempotent_and_newer_versions_are_preserved() {
        let mut c = Connection::open_in_memory().unwrap();
        run(&mut c).unwrap();
        run(&mut c).unwrap();
        assert_eq!(
            c.query_row("SELECT count(*) FROM schema_migrations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            3
        );
        c.pragma_update(None, "user_version", 99).unwrap();
        assert!(run(&mut c).unwrap_err().contains("update Lumen"));
        assert_eq!(
            c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            99
        );
    }
    #[test]
    fn a_failed_upgrade_rolls_back_every_step() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,applied_at INTEGER NOT NULL); INSERT INTO schema_migrations VALUES(2,0);").unwrap();
        assert!(run(&mut c).is_err());
        assert!(!column(&c, "sessions", "workspace_id").unwrap());
        assert_eq!(
            c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
}
