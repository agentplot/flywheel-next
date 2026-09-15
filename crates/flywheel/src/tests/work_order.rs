//! What a work order gives a session to report with: the exact command for
//! every report it may make, from its place (65, 67, 89).

use crate::host::{how_to_report, Reporting};
use std::path::Path;

fn reporting<'a>(session: &'a str, deliverables: &'a [String]) -> Reporting<'a> {
    Reporting {
        session,
        state: "/hosts/laptop/scratch/flywheel-state/main",
        manifest: Some(Path::new("/flywheel/flywheel.yaml")),
        page: Some("http://127.0.0.1:4242"),
        flywheel: "/bin/flywheel",
        host: "laptop",
        deliverables,
    }
}

/// Every session may offer a finding or a chore outside its job, so every order
/// gives the offer's command with the manifest a chore's scope is checked
/// against; the ask's command is a curation session's and not a chore's (58,
/// 60, 62, 116, `sessions.yaml` commands.offer, commands.ask).
#[test]
fn every_order_gives_the_offer_command_and_curations_the_ask() {
    let named = vec!["commits".to_string(), "verdict".to_string()];
    let chore = "work-item/flywheel-next/chore-1/wi-1/fix/1";
    let curation = "curation/scratch/main/1";
    for session in [chore, curation] {
        let said = how_to_report(&reporting(session, &named));
        assert!(
            said.contains("/bin/flywheel exit done --deliverable commits --deliverable verdict --host laptop"),
            "{said}"
        );
        let offer = said
            .lines()
            .find(|line| line.contains("/bin/flywheel offer "))
            .unwrap_or_else(|| panic!("the order gives no offer command: {said}"));
        assert_eq!(
            offer.trim(),
            format!(
                "FLYWHEEL_SESSION={session} FLYWHEEL_STATE=/hosts/laptop/scratch/flywheel-state/main \
                 FLYWHEEL_MANIFEST=/flywheel/flywheel.yaml FLYWHEEL_PAGE=http://127.0.0.1:4242 \
                 /bin/flywheel offer finding|chore --document <path> --about <object> [--scope bolt-line|<repository>] --host laptop"
            ),
        );
    }
    assert!(!how_to_report(&reporting(chore, &named)).contains(" ask "), "a chore's order gives the ask");
    assert!(
        how_to_report(&reporting(curation, &named)).contains("/bin/flywheel ask <repository> \"<the words>\" --host laptop"),
        "curation's order gives no ask"
    );
}
