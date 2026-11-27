//! Isolated backend fixtures: no real accounts, user configuration or project files.
use crate::*;

pub(crate) struct Fixture {
    pub core: Arc<Core>,
    pub root: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!("lumen-regression-{}", models::id()));
        std::fs::create_dir_all(&root).unwrap();
        let core = Arc::new(Core::new(
            store::Store::open(std::path::Path::new(":memory:")).unwrap(),
            root.clone(),
            Default::default(),
            None,
        ));
        Self { core, root }
    }

    pub fn directory(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        dunce::canonicalize(path).unwrap()
    }

    pub fn options(access: AccessMode) -> RunOptions {
        serde_json::from_value(serde_json::json!({
            "mode": "auto", "accountId": null, "model": null, "effort": "",
            "accessMode": access
        }))
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let temp = dunce::canonicalize(std::env::temp_dir()).unwrap();
        let root = dunce::canonicalize(&self.root).unwrap();
        assert!(root.starts_with(&temp) && root != temp);
        // Windows releases the handles of a just-killed process tree (such as its working
        // directory) asynchronously, so removal can briefly fail with a sharing violation.
        for _ in 0..50 {
            match std::fs::remove_dir_all(&root) {
                Err(e)
                    if e.raw_os_error() == Some(32)
                        || e.kind() == std::io::ErrorKind::PermissionDenied =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(100))
                }
                result => return result.unwrap(),
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

/// A test folder under the system temp directory, removed when it goes out of scope.
pub(crate) struct TempDir(PathBuf);
impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", models::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dunce::canonicalize(dir).unwrap())
    }
}
impl std::ops::Deref for TempDir {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}
impl AsRef<std::path::Path> for TempDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        // Best effort, as for `Fixture`: a just-ended child may hold a handle for a moment.
        for _ in 0..20 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
