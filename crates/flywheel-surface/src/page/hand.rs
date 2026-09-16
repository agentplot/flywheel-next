//! The operator's hand on a capture: its five controls, and the line under it
//! saying who reads it next and when (19a, S224, S224a).
//!
//! A capture is a note and never a decision: nothing about it is on the rail.
//! While a signal of it is unmoved it carries build now, add to bolt…, make an
//! intent, attach to… and drop, each a verb with its key calling its catalogue tool,
//! on the board, on its dock page and in the signals tray; once its signals
//! have moved it carries none (19a, 107, S220, S224). Which signals are
//! unmoved is read from the move records themselves, so a control used is gone
//! from the page that comes back rather than from the one after the next tick.

use super::{clipped, escape, Read};
use flywheel_domain::cadence;
use std::fmt::Write as _;

/// The signals a control on this object moves that nothing has moved yet: the
/// signal itself, or every signal of the capture.
pub fn unmoved(read: &Read, object: &str) -> Vec<String> {
    read.unmoved
        .iter()
        .filter(|signal| signal.id == object || signal.capture == object)
        .filter(|signal| still_unmoved(read, &signal.id))
        .map(|signal| signal.id.clone())
        .collect()
}

/// Every signal nothing has moved yet, once for the whole read, where a caller
/// asks after each of thousands (310a).
pub fn unmoved_ids(read: &Read) -> std::collections::BTreeSet<&str> {
    read.unmoved
        .iter()
        .filter(|signal| still_unmoved(read, &signal.id))
        .map(|signal| signal.id.as_str())
        .collect()
}

/// A signal whose machine has left `unmoved` has moved, whether or not its move
/// record is here to say so (107).
fn still_unmoved(read: &Read, signal: &str) -> bool {
    read.object(signal)
        .and_then(|o| o.config.get("move"))
        .is_none_or(|state| state == "unmoved")
}

/// The open bolts of the tracked repositories, in the order Construction shows
/// them: the board's groups in the model's order, and within a group the rows
/// as the status view holds them (S224a, 141). A bolt whose close is offered or
/// held is open and listed, since adding to it takes the offer back; one
/// landing, failed to land, landed or dropped is not.
pub fn open_bolts(read: &Read) -> Vec<String> {
    let open = |id: &str| {
        read.object(id).is_some_and(|o| o.config.get("life").map(String::as_str) == Some("open"))
    };
    let mut out: Vec<String> = Vec::new();
    for group in flywheel_domain::status::GROUPS {
        out.extend(
            read.status
                .rows
                .iter()
                .filter(|row| row.machine == "bolt" && row.group == group && open(&row.object))
                .map(|row| row.object.clone()),
        );
    }
    out
}

/// A bolt as a list shows it: its name, with its repository greyed before it
/// where the instance tracks more than one, as a slip and a chores card carry
/// it (S224a, S14).
fn bolt_named(read: &Read, id: &str) -> String {
    let name = read
        .object(id)
        .and_then(|bolt| bolt.record.get("name"))
        .and_then(|value| value.as_str())
        .map(String::from)
        .unwrap_or_else(|| id.rsplit('/').next().unwrap_or(id).to_string());
    let pre = match read.repositories.len() > 1 {
        true => id
            .strip_prefix("bolt/")
            .and_then(|rest| rest.split_once('/'))
            .map(|(repository, _)| format!("<span class=\"pre\">{} · </span>", escape(repository)))
            .unwrap_or_default(),
        false => String::new(),
    };
    format!("{pre}{}", escape(&clipped(&name)))
}

/// Who reads a waiting note next, and when, in the operator's words: how many
/// wait, and the curation record's threshold and cadence (110, 118, S224).
pub fn line(read: &Read) -> String {
    let waiting: usize = read.status.unmoved.iter().map(|source| source.count).sum();
    let Some(curation) = read.objects.iter().find(|o| o.machine == "curation") else {
        return format!("curation reads it next · {waiting} waiting");
    };
    if curation.config.get("run").map(String::as_str) == Some("running") {
        return format!("curation is reading it now · {waiting} waiting");
    }
    let threshold = curation.record.get("threshold").and_then(|v| v.as_u64()).unwrap_or(12) as usize;
    let schedule = curation.record.get("cadence").and_then(|v| v.as_str()).unwrap_or(cadence::DEFAULT);
    let ran = curation.entered_at.get("run").copied().unwrap_or(read.status.at);
    let when = match (waiting >= threshold, cadence::due(schedule, ran, read.status.at)) {
        (true, _) => "runs shortly".to_string(),
        (false, true) => "its schedule is up · runs shortly".to_string(),
        (false, false) => format!("runs at {threshold}, or when you run it"),
    };
    format!("curation reads it next · {waiting} waiting · {when}")
}

/// The five controls on a note whose signals nothing has moved, or nothing
/// once they have all moved (19a, S224, S224a). Each is a form posting to its tool,
/// with the verb as its label, the first free letter as its key and one line
/// of what follows as its title (S218, S220). A pick — the intent to attach
/// to, the repository to build in where there are several — is the whole
/// gesture: each choice is its own submit (S224).
pub fn controls(read: &Read, object: &str) -> String {
    if unmoved(read, object).is_empty() {
        return String::new();
    }
    let verbs = ["build now", "add to bolt", "make an intent", "attach to", "drop"].map(String::from);
    let keys = super::asks::keys(&verbs);
    let key = |at: usize| keys[at].map(|k| (format!(" data-key=\"{k}\""), format!("<span class=\"k\">{k}</span>"))).unwrap_or_default();
    let object = escape(object);
    let mut out = format!("<div class=\"answers hand\" data-hand=\"{object}\">\n");

    let (key_attr, key_hint) = key(0);
    let does = "a chore on a new bolt named from its words, started now";
    match read.repositories.as_slice() {
        [one] => {
            let _ = write!(
                out,
                "<form method=\"post\" action=\"/api/tools/propose-unit\" class=\"answer\">\
                 <input type=\"hidden\" name=\"capture\" value=\"{object}\">\
                 <input type=\"hidden\" name=\"repository\" value=\"{}\">\
                 <button type=\"submit\" class=\"btn sm\"{key_attr} title=\"{does}\">build now{key_hint}</button></form>\n",
                escape(one)
            );
        }
        many => {
            let _ = write!(
                out,
                "<details class=\"pick\"><summary{key_attr} title=\"{does}\">build now…{key_hint}</summary>\n"
            );
            match many.is_empty() {
                true => out.push_str(
                    "<div class=\"pick-list\"><p class=\"none\">Nothing to build in yet. Add a repository to flywheel.yaml.</p></div>\n",
                ),
                false => {
                    let _ = write!(
                        out,
                        "<form method=\"post\" action=\"/api/tools/propose-unit\" class=\"pick-list\">\
                         <input type=\"hidden\" name=\"capture\" value=\"{object}\">\n"
                    );
                    for repository in many {
                        let _ = write!(
                            out,
                            "<button type=\"submit\" name=\"repository\" value=\"{0}\">in {0}</button>\n",
                            escape(repository)
                        );
                    }
                    out.push_str("</form>\n");
                }
            }
            out.push_str("</details>\n");
        }
    }

    // A capture goes to a bolt as it goes to an intent: one already open that
    // the operator picks, the unit named from the capture's own words (34,
    // S224a). With none open the list says so and points at the verb that
    // makes one (S214).
    let (key_attr, key_hint) = key(1);
    let _ = write!(
        out,
        "<details class=\"pick\"><summary{key_attr} title=\"a chore on an open bolt you pick\">add to bolt…{key_hint}</summary>\n"
    );
    let bolts = open_bolts(read);
    match bolts.is_empty() {
        true => out.push_str(
            "<div class=\"pick-list\"><p class=\"none\">No bolt is open yet. Build now starts one.</p></div>\n",
        ),
        false => {
            let _ = write!(
                out,
                "<form method=\"post\" action=\"/api/tools/propose-unit\" class=\"pick-list\">\
                 <input type=\"hidden\" name=\"capture\" value=\"{object}\">\n"
            );
            for bolt in bolts {
                let _ = write!(
                    out,
                    "<button type=\"submit\" name=\"bolt\" value=\"{}\">{}</button>\n",
                    escape(&bolt),
                    bolt_named(read, &bolt)
                );
            }
            out.push_str("</form>\n");
        }
    }
    out.push_str("</details>\n");

    let (key_attr, key_hint) = key(2);
    let _ = write!(
        out,
        "<form method=\"post\" action=\"/api/tools/open-intent\" class=\"answer\">\
         <input type=\"hidden\" name=\"capture\" value=\"{object}\">\
         <button type=\"submit\" class=\"btn sm\"{key_attr} title=\"an intent named from its words, with this note on it\">\
         make an intent{key_hint}</button></form>\n"
    );

    let (key_attr, key_hint) = key(3);
    let _ = write!(
        out,
        "<details class=\"pick\"><summary{key_attr} title=\"put it on an intent that is open\">attach to…{key_hint}</summary>\n"
    );
    let open: Vec<(&str, String)> = read
        .objects
        .iter()
        .filter(|o| o.machine == "intent" && o.config.get("life").map(String::as_str) == Some("open"))
        .map(|o| {
            let subject = ["subject", "title"]
                .iter()
                .find_map(|field| o.record.get(*field).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()))
                .map(String::from)
                .unwrap_or_else(|| o.id.rsplit('/').next().unwrap_or(&o.id).replace('-', " "));
            (o.id.as_str(), subject)
        })
        .collect();
    match open.is_empty() {
        true => out.push_str(
            "<div class=\"pick-list\"><p class=\"none\">No intent is open yet. Make an intent from a note first.</p></div>\n",
        ),
        false => {
            let _ = write!(
                out,
                "<form method=\"post\" action=\"/api/tools/attach-signal\" class=\"pick-list\">\
                 <input type=\"hidden\" name=\"signal\" value=\"{object}\">\n"
            );
            for (intent, subject) in open {
                let _ = write!(
                    out,
                    "<button type=\"submit\" name=\"intent\" value=\"{}\">{}</button>\n",
                    escape(intent),
                    escape(&clipped(&subject))
                );
            }
            out.push_str("</form>\n");
        }
    }
    out.push_str("</details>\n");

    let (key_attr, key_hint) = key(4);
    let _ = write!(
        out,
        "<form method=\"post\" action=\"/api/tools/drop-signal\" class=\"answer\">\
         <input type=\"hidden\" name=\"signal\" value=\"{object}\">\
         <button type=\"submit\" class=\"btn sm drop\"{key_attr} title=\"set it aside; revive brings it back\">drop{key_hint}</button></form>\n"
    );
    out.push_str("</div>\n");
    out
}
