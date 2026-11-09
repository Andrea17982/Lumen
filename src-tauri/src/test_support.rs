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
        let core = Arc::new(Core {
            store: store::Store::open(std::path::Path::new(":memory:")).unwrap(),
            root: root.clone(),
            jobs: Mutex::new(HashMap::new()),
            slots: Arc::new(Semaphore::new(4)),
            inference: Arc::new(Semaphore::new(8)),
            workspace_locks: Mutex::new(HashMap::new()),
            catalogs: Mutex::new(HashMap::new()),
            usage_lock: Mutex::new(()),
            orchestra_raises: Mutex::new(HashMap::new()),
            limits: Default::default(),
            settings: Mutex::new(Default::default()),
            approvals: None,
        });
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
        std::fs::remove_dir_all(root).unwrap();
    }
}
