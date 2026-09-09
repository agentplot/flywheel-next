//! A state store for the tests: the git-only profile over a local bare
//! repository, so the catalogue is exercised against a store the phase admits
//! and never against a map of its own.
#![allow(dead_code)]

use chrono::{TimeZone, Utc};
use flywheel_store_git::{store::sandbox, GitStore};

pub struct Sandbox {
    dir: std::path::PathBuf,
}

impl Sandbox {
    pub fn new(name: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!(
            "flywheel-surface-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a sandbox");
        Sandbox { dir }
    }

    pub fn store(&self) -> GitStore {
        let at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        sandbox(&self.dir, "local", at).expect("a state repository and a host's checkout")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
