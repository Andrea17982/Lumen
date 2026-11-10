//! Bounded local data inspection; SQLite is opened read-only and statements cannot write.
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::Serialize;
#[derive(Serialize)]
pub struct Table {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    truncated: bool,
}
#[tauri::command]
pub async fn inspect_json(folder: String, file: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let path = crate::context::jail(std::path::Path::new(&folder), &file)?;
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
            return Err("JSON inspection is limited to 2 MB".into());
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
pub(crate) fn query(folder: &std::path::Path, file: &str, sql: &str) -> Result<Table, String> {
    let path = crate::context::jail(folder, file)?;
    if sql.len() > 16_000 {
        return Err("SQL is limited to 16,000 characters".into());
    }
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_millis(250))
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF;")
        .map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    db.progress_handler(
        1000,
        Some(move || started.elapsed() > std::time::Duration::from_millis(500)),
    );
    let mut statement = db.prepare(sql).map_err(|e| e.to_string())?;
    if !statement.readonly() || statement.column_count() == 0 {
        return Err("The inspector accepts read-only queries only".into());
    }
    let columns = statement
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    if columns.len() > 64 {
        return Err("Select no more than 64 columns".into());
    }
    let mut cursor = statement.query([]).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    let mut size = 0;
    let mut truncated = false;
    while let Some(row) = cursor.next().map_err(|e| e.to_string())? {
        if rows.len() >= 500 || size > 192_000 {
            truncated = true;
            break;
        }
        let mut cells = Vec::new();
        for i in 0..columns.len() {
            cells.push(match row.get_ref(i).map_err(|e| e.to_string())? {
                ValueRef::Null => serde_json::Value::Null,
                ValueRef::Integer(n) => serde_json::json!(n),
                ValueRef::Real(n) => serde_json::json!(n),
                ValueRef::Text(t) => {
                    let s = String::from_utf8_lossy(t);
                    if s.chars().count() > 2000 {
                        truncated = true;
                    }
                    serde_json::json!(s.chars().take(2000).collect::<String>())
                }
                ValueRef::Blob(b) => serde_json::json!(format!("<BLOB: {} bytes>", b.len())),
            });
        }
        size += serde_json::to_string(&cells)
            .map_err(|e| e.to_string())?
            .len();
        rows.push(cells);
    }
    Ok(Table {
        columns,
        rows,
        truncated,
    })
}
#[tauri::command]
pub async fn inspect_sqlite(folder: String, file: String, sql: String) -> Result<Table, String> {
    tokio::task::spawn_blocking(move || query(std::path::Path::new(&folder), &file, &sql))
        .await
        .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspection_cannot_write_or_escape_and_limits_results() {
        let f = crate::test_support::Fixture::new();
        let db = Connection::open(f.root.join("data.db")).unwrap();
        db.execute_batch("CREATE TABLE items(name TEXT); INSERT INTO items VALUES('Hero');")
            .unwrap();
        assert_eq!(
            query(&f.root, "data.db", "SELECT * FROM items")
                .unwrap()
                .rows[0][0],
            "Hero"
        );
        assert!(query(&f.root, "data.db", "DELETE FROM items RETURNING *").is_err());
        assert!(query(&f.root, "../other.db", "SELECT 1").is_err());
        assert!(query(
            &f.root,
            "data.db",
            "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT sum(x) FROM n"
        )
        .is_err());
        assert!(query(&f.root,"data.db","WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000) SELECT x FROM n").unwrap().truncated);
    }
}
