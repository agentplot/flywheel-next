//! The tour: an instance standing part-way through a scenario, and the
//! operator stepping it (`design/flywheel-next/scenarios/storefront.md`).
//!
//! A scenario applied into an instance leaves a fact behind saying which
//! scenario it was, how many actions it holds and how many have been played.
//! That fact is the whole of what makes the overlay appear: a host serving an
//! instance with actions left onboards whoever is looking at it, and one with
//! none serves the page and nothing else. There is no flag, no build feature
//! and no second code path — the page reads this the way it reads a lease or a
//! session, and the loop plays the next action the way it performs any other
//! act that has come due.
//!
//! It is a fact and not a record of a machine, because it is not something a
//! machine decides: it is what the run knows about itself, kept beside the
//! objects the way a place's and a session's facts are.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use flywheel_atoms::{Object, Records};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The fact one instance's tour is kept in.
pub fn tour_fact(instance: &str) -> String {
    format!("fact/tour/{instance}")
}

/// The held beat before a session's delivery appears (the design, "How an
/// action runs"): a deliberate pause so the viewer sees cause and effect,
/// rather than a progress bar pretending to be work.
pub const BEAT: i64 = 2;

/// An instance standing part-way through a scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tour {
    /// The scenario directory, so the loop can read the next action and the
    /// bundle beside it.
    pub scenario: String,
    /// What the scenario is called, for the overlay.
    pub title: String,
    /// How many actions it holds.
    pub actions: usize,
    /// How many have been played.
    pub played: usize,
    /// The copy for the action last played, which is what the board in front
    /// of the operator is showing.
    pub said: Option<String>,
    /// The copy for the action the next click plays.
    pub next: Option<String>,
    /// Whether the next action is a session's delivery, which is the one that
    /// holds a beat: the machinery has already stalled where the session would
    /// be working, and the pause is the viewer seeing that before the result.
    pub next_is_a_session: bool,
    /// When the machinery should play the action the operator asked for. None
    /// means nothing is owed and the tour waits on them.
    pub due_at: Option<DateTime<Utc>>,
}

impl Tour {
    /// Whether any action is left. The overlay is there while one is, and gone
    /// the moment none is.
    pub fn standing(&self) -> bool {
        self.played < self.actions
    }

    /// Whether the machinery owes an action the operator has asked for.
    ///
    /// The presence of the moment is the whole of it, and no clock is read
    /// here: the loop clears it as it plays the action, so a page that finds
    /// it set is a page whose operator has clicked and whose machinery has not
    /// finished. Compared against the page's own as-of instead, the beat was
    /// over before it began — a served host sets its clock at the top of a
    /// pass, so between passes the state's as-of is up to a poll behind the
    /// wall, and every beat was already in the past.
    pub fn working(&self) -> bool {
        self.due_at.is_some()
    }

    /// Whether an action is due to be played now.
    pub fn due(&self, now: DateTime<Utc>) -> bool {
        self.due_at.is_some_and(|due| due <= now) && self.standing()
    }

    fn record(&self) -> BTreeMap<String, Value> {
        [
            ("scenario".to_string(), json!(self.scenario)),
            ("title".to_string(), json!(self.title)),
            ("actions".to_string(), json!(self.actions)),
            ("played".to_string(), json!(self.played)),
            ("said".to_string(), json!(self.said)),
            ("next".to_string(), json!(self.next)),
            ("next_is_a_session".to_string(), json!(self.next_is_a_session)),
            (
                "due_at".to_string(),
                match self.due_at {
                    Some(at) => json!(at.to_rfc3339()),
                    None => Value::Null,
                },
            ),
        ]
        .into_iter()
        .collect()
    }
}

/// The tour of the instance this store holds, where there is one.
pub fn read<S: Records>(store: &S, instance: &str) -> Option<Tour> {
    let held = store.get(&tour_fact(instance)).ok().flatten()?;
    let text = |name: &str| {
        held.record
            .get(name)
            .and_then(|v| v.as_str())
            .map(String::from)
    };
    let count = |name: &str| {
        held.record
            .get(name)
            .and_then(|v| v.as_u64())
            .unwrap_or_default() as usize
    };
    Some(Tour {
        scenario: text("scenario")?,
        title: text("title").unwrap_or_default(),
        actions: count("actions"),
        played: count("played"),
        said: text("said"),
        next: text("next"),
        next_is_a_session: held
            .record
            .get("next_is_a_session")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        due_at: text("due_at").and_then(|at| {
            DateTime::parse_from_rfc3339(&at)
                .ok()
                .map(|t| t.with_timezone(&Utc))
        }),
    })
}

/// Write it back, leaving nothing else on the fact behind.
pub fn write<S: Records>(store: &mut S, instance: &str, tour: &Tour) -> Result<()> {
    let id = tour_fact(instance);
    let seq = store.get(&id)?.map(|o| o.seq).unwrap_or(0);
    let object = Object {
        id: id.clone(),
        machine: "fact".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: tour.record(),
        counters: Default::default(),
        applied_responses: vec![],
        seq,
        created: 0,
    };
    store.put(&id, &object, seq)?;
    Ok(())
}

/// The operator asked for the next action.
///
/// A session's delivery holds the beat first: the machinery has already
/// stalled where the session would be working, and the pause is the viewer
/// seeing that before the artifact appears. Anything else — something
/// arriving, an answer given — happens the moment it is asked for, because
/// that is what it is.
pub fn ask<S: Records>(store: &mut S, instance: &str, now: DateTime<Utc>) -> Result<Option<Tour>> {
    let Some(mut tour) = read(store, instance) else {
        return Ok(None);
    };
    if !tour.standing() || tour.due_at.is_some() {
        return Ok(Some(tour));
    }
    tour.due_at = Some(match tour.next_is_a_session {
        true => now + Duration::seconds(BEAT),
        false => now,
    });
    write(store, instance, &tour)?;
    Ok(Some(tour))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flywheel_atoms::testing::FakeStore;
    use chrono::TimeZone;

    fn tour() -> Tour {
        Tour {
            scenario: "scenarios/storefront".into(),
            title: "a team building a shop".into(),
            actions: 15,
            played: 2,
            said: Some("A transcript lands.".into()),
            next: Some("A reader goes through it.".into()),
            next_is_a_session: true,
            due_at: None,
        }
    }

    /// The fact is the whole of what says the overlay is there: it survives a
    /// write and a read, and an instance with none has none.
    #[test]
    fn a_tour_is_a_fact_of_the_instance() {
        let mut store = FakeStore::default();
        assert_eq!(read(&store, "storefront"), None, "no scenario, no overlay");
        write(&mut store, "storefront", &tour()).unwrap();
        assert_eq!(read(&store, "storefront"), Some(tour()));
    }

    /// The overlay stands while an action is left and goes when none is.
    #[test]
    fn it_stands_while_an_action_is_left() {
        let mut held = tour();
        assert!(held.standing());
        held.played = held.actions;
        assert!(!held.standing(), "played out: the overlay is not there");
    }

    /// A session's delivery holds the beat; anything else is played at once.
    #[test]
    fn only_a_session_holds_the_beat() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
        let mut store = FakeStore::default();
        write(&mut store, "storefront", &tour()).unwrap();

        let asked = ask(&mut store, "storefront", now).unwrap().unwrap();
        assert_eq!(asked.due_at, Some(now + Duration::seconds(BEAT)));
        assert!(asked.working(), "the agent is working, and is seen to be");
        assert!(!asked.due(now), "and nothing is played until the beat is out");
        assert!(asked.due(now + Duration::seconds(BEAT)));

        // Asking twice while one is owed changes nothing: a second click
        // during the beat is not a second action.
        let again = ask(&mut store, "storefront", now + Duration::seconds(1))
            .unwrap()
            .unwrap();
        assert_eq!(again.due_at, asked.due_at);

        // Something arriving, or an answer given, happens when it is asked.
        let mut store = FakeStore::default();
        let mut held = tour();
        held.next_is_a_session = false;
        write(&mut store, "storefront", &held).unwrap();
        let asked = ask(&mut store, "storefront", now).unwrap().unwrap();
        assert_eq!(asked.due_at, Some(now));
        assert!(asked.due(now));
        // Owed, and owed now: the page shows the beat for the moment it takes
        // the loop to play it and no longer.
        assert!(asked.working());
    }
}
