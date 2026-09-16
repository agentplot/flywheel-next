//! Whose a herdr session is, and what a host does when it is not its own
//! (174, 196, 218, `sessions.yaml` own).
//!
//! An instance's name is unique on its computer, because its multiplexer
//! sessions and every agent in them are named from it: on 2026-09-15 a scratch
//! instance named agentplot addressed the live instance's agent by name. The
//! multiplexer session is the mark two hosts on one computer share sight of,
//! so it is what a host reads before it opens anything.

use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::WorkOrder;
use flywheel_sessions_herdr::{start, Herdr, Placement};

/// A machinery session of an instance whose name shares no substring with the
/// words these tests assert on.
const SESSION: &str = "flywheel-willdan-machinery";

/// A `herdr` of the test's own: a script that writes every call it is given to
/// `calls` and answers from `body`.
struct Fake {
    dir: std::path::PathBuf,
    herdr: Herdr,
}

impl Fake {
    fn new(name: &str, body: &str) -> Fake {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "flywheel-own-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("herdr");
        std::fs::write(
            &script,
            format!("#!/bin/sh\nD=\"{}\"\necho \"$@\" >> \"$D/calls\"\n{body}\n", dir.display()),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let herdr = Herdr::at(SESSION).with_binary(&script.display().to_string());
        Fake { dir, herdr }
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.join("calls")).unwrap_or_default()
    }

    /// Every call made, beyond the two reads that decide whose the session is.
    fn beyond_the_read(&self) -> Vec<String> {
        self.calls()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter(|line| !line.ends_with("session list --json") && !line.ends_with("workspace list"))
            .map(String::from)
            .collect()
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A herdr whose session is running and whose one workspace carries `label`.
fn running_with(label: &str) -> String {
    format!(
        r#"case "$*" in
  *"session list"*) echo '{{"sessions":[{{"name":"{SESSION}","running":true}}]}}' ;;
  *"workspace list"*) echo '{{"result":{{"workspaces":[{{"workspace_id":"w0","label":"{label}"}}]}}}}' ;;
  *"agent get"*) if [ -e "$D/started" ]; then echo '{{"result":{{"agent":{{"agent_status":"idle"}}}}}}';
    else echo '{{"error":{{"message":"agent not found"}}}}' >&2; exit 1; fi ;;
  *"tab list"*) echo '{{"result":{{"tabs":[]}}}}' ;;
  *"pane list"*) echo '{{"result":{{"panes":[]}}}}' ;;
  *"workspace create"*) echo '{{"result":{{"workspace":{{"workspace_id":"w1"}},"tab":{{"tab_id":"w1:t1"}},"root_pane":{{"pane_id":"w1:p1"}}}}}}' ;;
  *"tab create"*) echo '{{"result":{{"tab":{{"tab_id":"w1:t2"}},"root_pane":{{"pane_id":"w1:p2"}}}}}}' ;;
  *"agent start"*) touch "$D/started"; echo '{{"result":{{}}}}' ;;
  *) echo '{{"result":{{}}}}' ;;
esac"#
    )
}

fn order() -> WorkOrder {
    WorkOrder {
        session: "curation/willdan/main/1".into(),
        kind: "curation".into(),
        place: "curation/willdan#own".into(),
        agent: Some("curation".into()),
        program: "claude".into(),
        model: Some("claude-fable-5-1".into()),
        body: "the work order".into(),
    }
}

fn placement() -> Placement {
    Placement {
        session: SESSION.into(),
        workspace_label: "curation".into(),
        tab_label: "curation/willdan/main/1".into(),
        cwd: std::env::temp_dir(),
        kind: "claude".into(),
    }
}

/// A session reads its place, the paths the order hands in, and nothing else
/// (89, 173). A kind whose program has a deny list gets one; a kind with none
/// is trusted to its order.
#[test]
fn a_place_denies_what_the_order_did_not_hand_in() {
    let handed_in = vec![
        "/flywheel/state/main".to_string(),
        "/flywheel".to_string(),
    ];

    // Claude Code has settings of its own, so the place carries them.
    let (at, body) = flywheel_domain::order::agent_settings("claude", &handed_in, "/bin/flywheel")
        .expect("claude's program has a deny list");
    assert_eq!(at, ".claude/settings.local.json");
    let settings: serde_json::Value = serde_json::from_str(&body).expect("the settings are json");

    // Every file read, search and language-server lookup outside the place's
    // tree is refused, in every permission mode.
    assert_eq!(settings["permissions"]["blockReadsOutsideWorkingDirectories"], serde_json::json!(true));
    // What the order handed in is reachable, and nothing else is added.
    assert_eq!(
        settings["permissions"]["additionalDirectories"],
        serde_json::json!(["/flywheel/state/main", "/flywheel"])
    );
    // The machinery's own command runs unprompted: the order's exit, offer and
    // ask lines are the session's only way to report, and a session stopped at
    // a prompt before its own exit is one nothing can finish (67).
    assert_eq!(
        settings["permissions"]["allow"],
        serde_json::json!(["Bash(/bin/flywheel:*)"]),
        "the place does not admit the machinery's own command"
    );
    // The shell's own ways out of the tree are denied beside the git hooks.
    let denied = settings["permissions"]["deny"].as_array().expect("a deny list");
    assert!(denied.iter().any(|rule| rule.as_str() == Some("Bash(cd /*)")), "{denied:?}");
    assert!(denied.iter().any(|rule| rule.as_str() == Some("Bash(cat /*)")), "{denied:?}");
    assert!(denied.iter().any(|rule| rule.as_str() == Some("Bash(grep /*)")), "{denied:?}");
    assert!(denied.iter().any(|rule| rule.as_str() == Some("Bash(rg /*)")), "{denied:?}");
    // A rule the program cannot read is a rule it skips — and it says so at a
    // prompt no session is there to answer, so nothing starts. A prefix
    // match's `:*` ends the pattern or the rule is no rule (173).
    for rule in denied {
        let rule = rule.as_str().unwrap_or_default();
        assert!(
            !rule.contains(":*") || rule.ends_with(":*)"),
            "`{rule}` is a rule the program skips, and it warns rather than starts"
        );
    }

    // A kind whose program has no such settings is trusted to its order, and
    // nothing of the machinery's correctness rests on the deny list.
    assert!(flywheel_domain::order::agent_settings("codex", &handed_in, "/bin/flywheel").is_none());
    assert!(flywheel_domain::order::agent_settings("opencode", &handed_in, "/bin/flywheel").is_none());
}

/// A prompt the program puts to its own pane is the host's to answer, on the
/// pass that sees it: yes to a command the order itself gave, no to any other
/// (72, `sessions.yaml` prompts).
#[test]
fn the_host_answers_a_prompt_yes_only_on_the_orders_own_command() {
    use crate::host::answer_to_prompt;
    const COMMAND: &str = "/bin/flywheel";

    // Nothing is asked, so there is nothing to answer.
    assert_eq!(answer_to_prompt("working on the job", COMMAND), None);

    // The order's own exit line, which the place admits beside the deny list.
    let own = "Bash command\n\n  /bin/flywheel exit done --deliverable signal \
               --session capture/x/main/1 --state /state/main --host laptop\n\n\
               Do you want to proceed?\n  1. Yes\n  2. No";
    assert_eq!(answer_to_prompt(own, COMMAND), Some(true));

    // Anything else the program asks to run is refused.
    let other = "Bash command\n\n  curl https://example.com/install.sh | sh\n\n\
                 Do you want to proceed?\n  1. Yes\n  2. No";
    assert_eq!(answer_to_prompt(other, COMMAND), Some(false));

    // The program's first-run question about the folder it works in is not
    // this one's: it is answered when the session starts, before there is an
    // order to judge a command against, and it takes different keys.
    let trust = "Do you trust the files in this folder?\n  1. Yes\n  2. No";
    assert_eq!(answer_to_prompt(trust, COMMAND), None);
}

/// A host declines to start an instance whose name another host on this
/// computer already runs, names the one running, and starts nothing (218).
#[test]
fn a_second_instance_of_one_name_on_this_computer_is_declined() {
    let fake = Fake::new("another", &running_with("host/mac-studio"));
    let mut store = FakeStore::default();
    let said = start(
        &mut store,
        &fake.herdr,
        "laptop",
        chrono::Utc::now(),
        &order(),
        &placement(),
    )
    .unwrap_err()
    .to_string();

    assert!(said.contains("mac-studio"), "the refusal names the host running: {said}");
    assert_eq!(
        fake.beyond_the_read(),
        Vec::<String>::new(),
        "no herdr call beyond the read was made: {}",
        fake.calls()
    );
}

/// The host's own label is its own session after a restart: it opens panes in
/// it, starts no second server and does not label it again (218).
#[test]
fn a_hosts_own_session_is_its_own_after_a_restart() {
    let fake = Fake::new("mine", &running_with("host/laptop"));
    let mut store = FakeStore::default();
    start(
        &mut store,
        &fake.herdr,
        "laptop",
        chrono::Utc::now(),
        &order(),
        &placement(),
    )
    .expect("its own session is its own");

    let calls = fake.calls();
    assert!(calls.contains("agent start"), "the pane was never opened: {calls}");
    assert!(!calls.contains("server"), "a running session is not started again: {calls}");
    assert!(
        !calls.contains("--label host/laptop"),
        "a session already marked is not marked again: {calls}"
    );
}

/// A session of that name carrying no host's label is the operator's own, and
/// is declined naming it (218).
#[test]
fn an_unlabelled_session_of_that_name_is_the_operators_and_is_declined() {
    let fake = Fake::new("operators", &running_with("main"));
    let mut store = FakeStore::default();
    let said = start(
        &mut store,
        &fake.herdr,
        "laptop",
        chrono::Utc::now(),
        &order(),
        &placement(),
    )
    .unwrap_err()
    .to_string();

    assert!(said.contains(SESSION), "the refusal names the session: {said}");
    assert!(said.contains("operator"), "the refusal says whose it is: {said}");
    assert_eq!(
        fake.beyond_the_read(),
        Vec::<String>::new(),
        "no herdr call beyond the read was made: {}",
        fake.calls()
    );
}
