//! The scripted sessions play a session's reports by running the command a real
//! session runs; where that command runs from is this crate's own (67, 93, D8).

use crate::sessions::ScriptedSessions;
use std::path::PathBuf;

const SESSION: &str = "elaboration/a/1/self-closing/1";

#[test]
fn the_command_runs_from_the_session_place() {
    let sessions = ScriptedSessions::new("/tmp/state", "/tmp/places");
    assert_eq!(
        sessions.place(SESSION),
        PathBuf::from("/tmp/places/elaboration-a-1-self-closing-1"),
        "the command's working directory is the session's place (D8)"
    );
}
