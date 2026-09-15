//! Herdr's agents, watched: an agent's change of state is a cause now and not
//! at the next poll (S221, 130, D6).

use crate::watch::Watchers;
use flywheel_atoms::testing::FakeStore;
use flywheel_sessions_herdr::Herdr;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

/// A `herdr` of the test's own: the agent reads as working, its first wait
/// returns it idle, and a wait after that finds it gone.
struct Fake {
    dir: std::path::PathBuf,
    herdr: Herdr,
}

impl Fake {
    fn new(name: &str) -> Fake {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "flywheel-watch-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("herdr");
        // Every call names the herdr session the pane is in, so the agent's
        // name is no longer the second word (174).
        let body = r#"case "$*" in
  *"agent get"*) echo '{"result":{"agent":{"agent_status":"working"}}}' ;;
  *"agent wait"*)
    if [ -e "$D/waited" ]; then echo '{"error":{"message":"agent not found"}}' >&2; exit 1; fi
    touch "$D/waited"
    echo '{"result":{"agent":{"agent_status":"idle"}}}' ;;
  *) exit 1 ;;
esac"#;
        std::fs::write(&script, format!("#!/bin/sh\nD=\"{}\"\n{body}\n", dir.display())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let herdr = Herdr::at("flywheel-willdan-bolts").with_binary(&script.display().to_string());
        Fake { dir, herdr }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A live agent going idle raises the generation every open page listens for
/// and wakes the loop, the moment Herdr says so (S221, 130).
#[test]
fn an_agents_change_of_state_raises_the_generation_and_wakes_the_loop() {
    let fake = Fake::new("change");
    let mut store = FakeStore::default();
    flywheel_sessions_operator::set(
        &mut store,
        "work-item/atlas/rows/wi-1/fix/1",
        &[
            ("started_at", json!("2026-09-14T12:00:00Z")),
            ("ended_at", Value::Null),
            ("host", json!("laptop")),
            ("herdr_agent", json!("rows-wi-1-fix-1")),
            // The herdr session the pane is in: what the wait addresses (174).
            ("herdr_session", json!("flywheel-willdan-bolts")),
        ],
    )
    .unwrap();

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let woken = Arc::new(tokio::sync::Notify::new());
        let changed = Arc::new(tokio::sync::watch::Sender::new(1u64));
        let mut generation = changed.subscribe();
        let wake = woken.notified();
        tokio::pin!(wake);
        wake.as_mut().enable();

        Watchers::over(fake.herdr.clone(), Duration::ZERO, woken.clone(), changed.clone()).follow(&store);

        tokio::time::timeout(Duration::from_secs(5), generation.changed())
            .await
            .expect("the change was heard")
            .expect("the generation is still sent");
        assert_eq!(*generation.borrow(), 2);
        tokio::time::timeout(Duration::from_secs(5), wake)
            .await
            .expect("the loop was woken");
    });
}
