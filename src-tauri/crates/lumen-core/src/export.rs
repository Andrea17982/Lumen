//! A conversation as one plain-text file: every message with its full activity (tool calls and
//! their output, errors, execution diagnostics), as the service stored it. Agents never see
//! this history, so the export is the service's job, written where the user chose.
use crate::{models::Activity, CoreRef, Session};
use std::fmt::Write as _;
use std::path::Path;

pub(crate) struct Row {
    pub role: String,
    pub text: String,
    pub activity: Vec<Activity>,
    pub created_at: i64,
}

fn time(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        })
        .unwrap_or_else(|| ms.to_string())
}

fn indented(out: &mut String, text: &str) {
    for line in text.lines() {
        let _ = writeln!(out, "    {line}");
    }
}

pub(crate) fn render(session: &Session, rows: &[Row], exported_ms: i64) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Lumen conversation export");
    let _ = writeln!(out, "Title:    {}", session.title);
    let _ = writeln!(out, "Session:  {}", session.id);
    let _ = writeln!(out, "Status:   {}", session.status);
    let _ = writeln!(out, "Folder:   {}", session.workspace);
    let _ = writeln!(out, "Exported: {}", time(exported_ms));
    let _ = writeln!(out, "Messages: {}", rows.len());
    for (index, row) in rows.iter().enumerate() {
        let who = if row.role == "user" { "You" } else { "Lumen" };
        let _ = writeln!(
            out,
            "\n=== {} · {} · {} ===",
            index + 1,
            who,
            time(row.created_at)
        );
        if row.text.trim().is_empty() {
            let _ = writeln!(out, "(no text)");
        } else {
            let _ = writeln!(out, "{}", row.text.trim_end());
        }
        if !row.activity.is_empty() {
            let _ = writeln!(out, "\n--- Activity ({}) ---", row.activity.len());
            for item in &row.activity {
                let _ = writeln!(out, "[{}]", item.kind);
                indented(&mut out, &item.text);
            }
        }
    }
    out
}

/// Only a `.txt` file in an existing folder, named by an absolute path.
fn target(path: &str) -> Result<&Path, String> {
    let file = Path::new(path);
    let txt = file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("txt"));
    // A `:` in the name would be an NTFS stream of another file; a link would write elsewhere.
    let stream = file
        .file_name()
        .is_some_and(|n| n.to_string_lossy().contains(':'));
    let link = std::fs::symlink_metadata(file).is_ok_and(|m| m.file_type().is_symlink());
    if !file.is_absolute() || !txt || stream || link || file.is_dir() {
        return Err("Choose a .txt file to export the conversation to".into());
    }
    if !file.parent().is_some_and(Path::is_dir) {
        return Err("The folder for the export does not exist".into());
    }
    Ok(file)
}

/// The desktop shell asks where to save and passes that path; the page never names one.
#[crate::command]
pub(crate) fn export_session(core: CoreRef<'_>, id: String, path: String) -> Result<(), String> {
    let file = target(&path)?;
    let session = core.store.session(&id)?;
    let rows = core.store.with(|db| {
        let mut q = db.prepare(
            "SELECT role,text,activity,created_at FROM messages WHERE session_id=? ORDER BY created_at,rowid",
        )?;
        let rows = q.query_map([&id], |r| {
            let activity: String = r.get(2)?;
            Ok(Row {
                role: r.get(0)?,
                text: r.get(1)?,
                // A row the client could not parse would also be unreadable here: keep the rest.
                activity: serde_json::from_str(&activity).unwrap_or_else(|_| {
                    vec![Activity {
                        kind: "unreadable-activity".into(),
                        text: activity.clone(),
                    }]
                }),
                created_at: r.get(3)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    })?;
    let text = render(&session, &rows, chrono::Utc::now().timestamp_millis());
    // Written beside the target, then renamed: a failure never leaves a half-replaced file.
    let partial = file.with_extension(format!("txt.{}.partial", uuid::Uuid::new_v4()));
    let written = std::fs::write(&partial, text).and_then(|()| std::fs::rename(&partial, file));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(format!("Could not write the export: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            id: "s1".into(),
            title: "Release 0.17".into(),
            pinned: false,
            status: "partial".into(),
            workspace: "C:/work".into(),
            updated_at: 0,
            workspace_id: None,
            access_mode: crate::models::AccessMode::Autopilot,
        }
    }

    #[test]
    fn keeps_every_message_tool_call_and_error() {
        let rows = vec![
            Row {
                role: "user".into(),
                text: "Push and release".into(),
                activity: vec![],
                created_at: 1_795_878_031_279,
            },
            Row {
                role: "assistant".into(),
                text: String::new(),
                activity: vec![
                    Activity {
                        kind: "tool".into(),
                        text: "gh pr checks 35\nweb pass".into(),
                    },
                    Activity {
                        kind: "error".into(),
                        text: "Claude returned no assistant text".into(),
                    },
                ],
                created_at: 1_795_878_031_291,
            },
        ];
        let text = render(&session(), &rows, 1_795_881_102_318);
        assert!(text.starts_with("Lumen conversation export\nTitle:    Release 0.17\n"));
        assert!(text.contains("Messages: 2"));
        assert!(text.contains("=== 1 · You · "));
        assert!(text.contains("Push and release"));
        assert!(text.contains("=== 2 · Lumen · "));
        assert!(text.contains("(no text)"));
        assert!(text.contains("--- Activity (2) ---\n[tool]\n    gh pr checks 35\n    web pass\n"));
        assert!(text.contains("[error]\n    Claude returned no assistant text\n"));
    }

    #[test]
    fn writes_only_a_txt_file_in_an_existing_folder() {
        let dir = std::env::temp_dir();
        let ok = dir.join("lumen-export-check.txt");
        assert!(target(ok.to_str().unwrap()).is_ok());
        assert!(target("relative.txt").is_err());
        assert!(target(dir.join("export.exe").to_str().unwrap()).is_err());
        assert!(target(
            dir.join("missing-folder-7f3a")
                .join("x.txt")
                .to_str()
                .unwrap()
        )
        .is_err());
        assert!(target(dir.to_str().unwrap()).is_err());
        assert!(target(dir.join("a.txt:stream.txt").to_str().unwrap()).is_err());
    }
}
