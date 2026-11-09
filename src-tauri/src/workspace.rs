//! Explicit chat-level directory changes. Only the current user's directive is parsed;
//! paths mentioned in history, attachments or tool output never rebind a session.
use crate::{models::Session, Core};
use std::path::{Path, PathBuf};

pub(crate) fn requested_folder(text: &str) -> Result<Option<String>, String> {
    let directive = regex::Regex::new(concat!(
        r"(?i)^(?:change:\s*)?(?:",
        r"/workspace\s+|",
        r"(?:cambia|imposta|usa)\s+(?:la\s+)?(?:directory(?:\s+di\s+lavoro)?|cartella(?:\s+di\s+lavoro)?|workspace)\s*(?:(?:in|a|su)\s+|:\s*)?|",
        r"(?:change|set|use)\s+(?:the\s+)?(?:working\s+directory|workspace|folder)\s*(?:to\s+|:\s*)?|",
        r"(?:procedi[,\s]+(?:ovviamente\s+)?)?(?:il\s+progetto|[\p{L}\p{N}_-]+)\s+su\s+cui\s+lavorare\s+[èe]['’]?\s*(?:questo\s*)?:\s*",
        r")(.+)$"
    )).unwrap();
    // The directive must start the message. Later paths may be pasted reference material.
    if !text
        .trim_start()
        .lines()
        .next()
        .is_some_and(|line| directive.is_match(line.trim()))
    {
        return Ok(None);
    }
    let mut found = None;
    let drive_path = regex::Regex::new(r"^[A-Za-z]:[\\/]").unwrap();
    for line in text.lines() {
        let Some(c) = directive.captures(line.trim()) else {
            continue;
        };
        let rest = c[1].trim();
        let raw = if let Some(quote) = rest
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"' | '`'))
        {
            let tail = &rest[quote.len_utf8()..];
            let end = tail
                .find(quote)
                .ok_or("Close the quotes around the workspace path")?;
            &tail[..end]
        } else {
            rest.split(" (")
                .next()
                .unwrap_or(rest)
                .trim_end_matches(['.', ';'])
                .trim()
        };
        let absolute =
            Path::new(raw).is_absolute() || drive_path.is_match(raw) || raw.starts_with(r"\\");
        if !absolute {
            return Err("Use an absolute directory path for the conversation workspace".into());
        }
        if found.replace(raw.to_string()).is_some() {
            return Err("Specify one conversation workspace at a time".into());
        }
    }
    Ok(found)
}

pub(crate) fn resolve_folder(text: &str) -> Result<Option<PathBuf>, String> {
    requested_folder(text)?
        .map(|folder| {
            if !Path::new(&folder).is_absolute() {
                return Err("Use an absolute directory path for this operating system".into());
            }
            let path = dunce::canonicalize(&folder).map_err(|_| {
                format!("Workspace directory does not exist or is inaccessible: {folder}")
            })?;
            if !path.is_dir() {
                return Err(format!("Workspace must be a directory: {folder}"));
            }
            Ok(path)
        })
        .transpose()
}

/// Shares the running-job guard with the folder picker: a live CLI never changes root.
pub(crate) fn bind(core: &Core, id: &str, path: &Path) -> Result<Session, String> {
    let jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
    if jobs.contains_key(id) {
        return Err("Stop this conversation before changing its folder".into());
    }
    let project = core
        .store
        .workspaces()?
        .into_iter()
        .find(|w| dunce::canonicalize(&w.folder).ok().as_deref() == Some(path));
    core.store.bind_workspace(
        id,
        project.as_ref().map(|w| w.id.as_str()),
        &path.to_string_lossy(),
    )
}

pub(crate) fn apply(
    app: &tauri::AppHandle,
    core: &Core,
    id: &str,
    text: &str,
) -> Result<bool, String> {
    let Some(path) = resolve_folder(text)? else {
        return Ok(false);
    };
    let session = core.store.session(id)?;
    if dunce::canonicalize(&session.workspace).ok().as_deref() == Some(path.as_path()) {
        return Ok(false);
    }
    let session = bind(core, id, &path)?;
    crate::event(
        app,
        id,
        "",
        "workspace",
        &serde_json::to_string(&session).map_err(|e| e.to_string())?,
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_directory_changes_preserve_spaces() {
        for text in [
            r"/workspace M:\Personal Projects\Lumen",
            r"Cambia directory di lavoro in M:\Personal Projects\Lumen",
            r#"Use workspace "M:\Personal Projects\Lumen""#,
            r"Change: Procedi, ovviamente Lumen su cui lavorare e' questo: M:\Personal Projects\Lumen (avevo sbagliato directory)",
        ] {
            assert_eq!(
                requested_folder(text).unwrap().as_deref(),
                Some(r"M:\Personal Projects\Lumen"),
                "{text}"
            );
        }
        #[cfg(unix)]
        assert_eq!(
            requested_folder("Set working directory to `/tmp/lumen`")
                .unwrap()
                .as_deref(),
            Some("/tmp/lumen")
        );
        #[cfg(windows)]
        assert!(requested_folder("Set working directory to `/tmp/lumen`").is_err());
    }

    #[test]
    fn references_and_quoted_instructions_do_not_switch_projects() {
        for text in [
            r"Leggi M:\Personal Projects\Lumen\README.md",
            r"> /workspace M:\Other",
            "```text\n/workspace /other\n```",
            "Ecco il documento:\n\n/workspace /other\nfine",
            r"Non cambiare workspace in M:\Other",
        ] {
            assert_eq!(requested_folder(text).unwrap(), None);
        }
    }

    #[test]
    fn invalid_targets_fail_instead_of_using_the_old_workspace() {
        assert!(requested_folder("/workspace relative/path").is_err());
        assert!(requested_folder("/workspace \"/unclosed").is_err());
        assert!(requested_folder("/workspace /one\n/workspace /two").is_err());
        assert!(resolve_folder("/workspace /lumen-nonexistent-test-directory-937481").is_err());
        let dir = std::env::temp_dir();
        let request = format!("/workspace \"{}\"", dir.display());
        assert_eq!(
            resolve_folder(&request).unwrap(),
            Some(dunce::canonicalize(dir).unwrap())
        );
    }

    #[test]
    fn switching_persists_the_root_and_project_and_refuses_a_running_turn() {
        let fixture = crate::test_support::Fixture::new();
        let old = fixture.directory("Rodent Heroes");
        let new = fixture.directory("Lumen");
        let session = fixture
            .core
            .store
            .create(old.to_string_lossy().into())
            .unwrap();
        let project = fixture
            .core
            .store
            .add_workspace("Lumen".into(), new.to_string_lossy().into())
            .unwrap();
        let (tx, _rx) = tokio::sync::watch::channel(false);
        fixture
            .core
            .jobs
            .lock()
            .unwrap()
            .insert(session.id.clone(), tx);
        assert!(bind(&fixture.core, &session.id, &new).is_err());
        assert_eq!(
            fixture.core.store.session(&session.id).unwrap().workspace,
            old.to_string_lossy()
        );
        fixture.core.jobs.lock().unwrap().clear();
        let switched = bind(&fixture.core, &session.id, &new).unwrap();
        assert_eq!(switched.workspace, new.to_string_lossy());
        assert_eq!(switched.workspace_id, Some(project.id));
        let unregistered = fixture.directory("Other project");
        let switched = bind(&fixture.core, &session.id, &unregistered).unwrap();
        assert_eq!(
            switched.workspace_id, None,
            "do not keep the old project's identity"
        );
        assert_eq!(
            fixture.core.store.session(&session.id).unwrap().workspace,
            unregistered.to_string_lossy()
        );
    }

    #[tokio::test]
    async fn switched_prompt_and_spill_files_use_only_the_new_project() {
        use crate::test_support::Fixture;
        let fixture = Fixture::new();
        let old = fixture.directory("Rodent Heroes");
        let new = fixture.directory("Lumen");
        crate::context::write_context(&old, "Rodent project instructions").unwrap();
        crate::context::write_context(&new, "Lumen project instructions").unwrap();
        let session = fixture
            .core
            .store
            .create(old.to_string_lossy().into())
            .unwrap();
        let session = bind(&fixture.core, &session.id, &new).unwrap();
        fixture
            .core
            .store
            .save_message(
                &session.id,
                &crate::models::Message {
                    id: crate::models::id(),
                    role: "user".into(),
                    text: "exported conversation line\n".repeat(450),
                    activity: vec![],
                },
            )
            .unwrap();
        let (prompt, _) = crate::turns::build_prompt(
            &fixture.core,
            &session,
            &[],
            &Fixture::options(crate::models::AccessMode::Autopilot),
        )
        .await
        .unwrap();
        assert!(prompt.contains(&format!(
            "Current conversation workspace: {}",
            new.display()
        )));
        assert!(prompt.contains("Lumen project instructions"));
        assert!(!prompt.contains("Rodent project instructions"));
        assert!(new.join(".lumen/spill").is_dir());
        assert!(!old.join(".lumen/spill").exists());
    }
}
