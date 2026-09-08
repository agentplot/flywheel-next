//! The six commands, over `scenarios/rail-mockup.yaml`, against a snapshot
//! captured before they were moved onto the trait surface. What the operator
//! sees must not move when the store behind it does (D3, D1).

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Every command in order, with its output, exactly as the snapshot holds it.
const RUN: &[&[&str]] = &[
    &["seed", "scenarios/rail-mockup.yaml"],
    &["rail"],
    &["tick", "1"],
    &["respond", "412", "yes"],
    &["dictate", "bolt/atlas/plan-rows", "land and follow"],
    &["log", "12"],
];

#[test]
fn cli_snapshot() {
    let dir = std::env::temp_dir().join(format!("flywheel-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let state = dir.join("store.json");

    let mut out = String::new();
    for args in RUN {
        out.push_str(&format!("$ flywheel {}\n", args.join(" ")));
        let result = Command::new(env!("CARGO_BIN_EXE_flywheel"))
            .current_dir(root())
            .arg("--state")
            .arg(&state)
            .args(*args)
            .output()
            .expect("the command runs");
        out.push_str(&String::from_utf8_lossy(&result.stdout));
        out.push_str(&String::from_utf8_lossy(&result.stderr));
        out.push('\n');
    }
    let _ = std::fs::remove_dir_all(&dir);

    let snapshot_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/cli.txt"
    ));
    let snapshot = std::fs::read_to_string(&snapshot_path).expect("the snapshot is committed");
    if out != snapshot {
        let actual = snapshot_path.with_extension("txt.actual");
        std::fs::write(&actual, &out).ok();
        panic!(
            "the commands' output moved. Written to {} — diff it against {}",
            actual.display(),
            snapshot_path.display()
        );
    }
}
