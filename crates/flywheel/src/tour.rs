//! Stepping a scenario from the page (`design/flywheel-next/scenarios/
//! storefront.md`, "How an action runs").
//!
//! `--through <n>` on a command line plays a scenario into an instance and
//! stops; that is the "applied whole" half of the design. This is the other
//! half: the operator advances one action at a time and watches the board
//! move.
//!
//! It is the same runner and the same actions. What a click does is write on
//! the tour's fact that the next action is owed, and the host's own loop plays
//! it on its next pass, exactly as it performs any other act that has come due
//! — so there is no second code path, no developer mode and no flag. Where the
//! action is a session's delivery the fact says it is owed two seconds from
//! now, and the page shows the agent working until then: the machinery has
//! already stalled where a real session would be, and the beat is the viewer
//! seeing that before the artifact appears.
//!
//! The overlay is the product's own onboarding, so what turns it on is state
//! and not a switch: an instance a scenario was applied into carries the fact,
//! and an instance that was not does not.

use crate::apply::{self, Instance};
use crate::host::Host;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::conformance::Action;
use flywheel_domain::tour::Tour;
use flywheel_scenario::conformance::Suite;
use std::path::{Path, PathBuf};

/// What one action is, for the page: the beat is a session's and no other's.
fn is_a_session(action: Option<&Action>) -> bool {
    matches!(action, Some(Action::Session(_)))
}

/// Write the tour's fact from a scenario and the point it stands at.
///
/// Called by an apply when it stops, and by each step as it lands, so the fact
/// always says what the operator is looking at and what the next click does.
pub fn record(host: &mut Host, scenario: &Path, played: usize) -> Result<()> {
    // Absolute: the fact is read back by a host serving from a working
    // directory of its own, so a path relative to the one that applied it
    // would name nothing (205).
    let scenario = &scenario
        .canonicalize()
        .with_context(|| format!("resolving {}", scenario.display()))?;
    let (read, _) = flywheel_atoms::conformance::load(scenario)?;
    let actions = read.actions()?;
    let tour = Tour {
        scenario: scenario.to_string_lossy().to_string(),
        title: read.title.clone(),
        actions: actions.len(),
        played,
        // The line for what just happened, which is what the board in front of
        // the operator is showing; and the line for what the next click does.
        said: read.tour_line(played).map(String::from),
        next: read.tour_line(played + 1).map(String::from),
        next_is_a_session: is_a_session(actions.get(played)),
        due_at: None,
    };
    let instance = host.instance.clone();
    flywheel_domain::tour::write(&mut host.store.git, &instance, &tour)
}

/// Play the action the operator asked for, if its moment has come.
///
/// Returns whether anything was played, so the loop can take another pass at
/// once rather than waiting out its poll.
pub fn play_due(host: &mut Host, now: DateTime<Utc>) -> Result<bool> {
    let instance = host.instance.clone();
    let Some(tour) = flywheel_domain::tour::read(&host.store.git, &instance) else {
        return Ok(false);
    };
    if !tour.due(now) {
        return Ok(false);
    }
    let scenario = PathBuf::from(&tour.scenario);
    let Some(manifest) = host.manifest.clone() else {
        anyhow::bail!(
            "this host does not know the manifest it was opened on, so it cannot reach the \
             instance a scenario is stepped into (205)"
        );
    };
    let at = Instance::of(&manifest, &host.name)?;
    let suite = Suite::for_scenario(&scenario)
        .with_context(|| format!("the suite closing {}", scenario.display()))?;
    let (read, _) = flywheel_atoms::conformance::load(&scenario)?;
    let actions = read.actions()?;
    let Some(action) = actions.get(tour.played) else {
        return Ok(false);
    };
    let bundle = flywheel_atoms::conformance::bundle_of(&scenario);
    let number = tour.played + 1;
    apply::play(host, &at, action, bundle.as_deref(), &suite)
        .with_context(|| format!("action {number}"))?;
    // The cascade the action started, run out before the operator is shown the
    // result: a moment they step to is a moment the machinery has arrived at.
    apply::settle(host)?;
    record(host, &scenario, number)?;
    Ok(true)
}

/// When the loop must next look, where the operator has asked for an action
/// whose beat has not run out. Nothing else in the loop has a moment of its
/// own, so this is the only thing that shortens a poll.
pub fn due_at(host: &Host) -> Option<DateTime<Utc>> {
    flywheel_domain::tour::read(&host.store.git, &host.instance).and_then(|t| t.due_at)
}
