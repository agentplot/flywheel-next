//! The signals tray: every signal curation has not read, grouped by capture and
//! ordered by source and age, each row a quote with the capture's four
//! controls, under a head that runs curation now beside what would run it on
//! its own (118, 19a, 110, S225). The tray asks nothing itself: what curation
//! proposes lands on the rail, one decision a proposal (109, 116). A finding a
//! session offered with nothing above it is a row like any other, its quote the
//! document's path and its line the session that offered it (62, S231).

use super::lists;
use super::{curator, escape, hand, source_name, Read};
use flywheel_domain::{cadence, signals};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The tray's surface in the dock, which the counter opens.
pub const ID: &str = "tray";

/// What waits, capture by capture, with each capture's signals nothing has
/// moved.
fn waiting(read: &Read) -> Vec<(&signals::Waiting, Vec<&signals::Signal>)> {
    let unmoved = hand::unmoved_ids(read);
    read.status
        .waiting
        .iter()
        .map(|waiting| {
            let rows = waiting.signals.iter().filter(|s| unmoved.contains(s.id.as_str())).collect::<Vec<_>>();
            (waiting, rows)
        })
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
}

fn curation(read: &Read) -> Option<&flywheel_engine::Object> {
    read.objects.iter().find(|o| o.machine == "curation")
}

fn running(read: &Read) -> bool {
    curation(read).and_then(|c| c.config.get("run")).is_some_and(|run| run == "running")
}

/// What would run curation on its own, in the operator's words: "runs on its
/// own at 12, or weekdays at 06:00" (110, S225).
pub fn trigger(read: &Read) -> String {
    let Some(curation) = curation(read) else {
        return String::new();
    };
    let threshold = curation.record.get("threshold").and_then(|v| v.as_u64()).unwrap_or(12);
    let schedule = curation.record.get("cadence").and_then(|v| v.as_str()).unwrap_or(cadence::DEFAULT);
    format!("runs on its own at {threshold}, or {}", cadence::in_words(schedule))
}

/// Curation's chip while it reads: the same chip a session shows on the board,
/// with the host it runs on and the link that says where its pane is (S53,
/// S234). A run the page knows no session record for shows what it is doing
/// and offers no link, since there is no pane to name.
fn reading_chip(read: &Read) -> String {
    let session = read.curation.as_deref().and_then(|id| read.sessions.get(id));
    let host = match session {
        Some(session) if !session.host.is_empty() => {
            format!("<span class=\"hs\">@{}</span>", super::escape(&session.host))
        }
        _ => String::new(),
    };
    let pane = match session {
        Some(session) => super::pane_link(session, &read.served_by),
        None => String::new(),
    };
    format!(
        "<span class=\"sc working\"><span class=\"dot working\"></span>\
         <span class=\"ag\">curation</span>{host}<span class=\"ac\">reading</span>{pane}</span>"
    )
}

/// How many wait, from which sources, and how old the oldest is.
fn summary(read: &Read, waiting: &[(&signals::Waiting, Vec<&signals::Signal>)]) -> (usize, String, Option<String>) {
    let mut by_source: BTreeMap<&str, usize> = BTreeMap::new();
    for (capture, rows) in waiting {
        *by_source.entry(source_name(&capture.source)).or_default() += rows.len();
    }
    let count = by_source.values().sum();
    let sources = by_source.iter().map(|(source, n)| format!("{source} {n}")).collect::<Vec<_>>().join(" · ");
    let oldest = waiting
        .iter()
        .filter_map(|(capture, _)| said_at(&capture.event_at))
        .min()
        .map(|at| match (read.status.at - at).num_days() {
            days if days < 1 => "today".to_string(),
            days => format!("{days}d"),
        });
    (count, sources, oldest)
}

/// When a capture's material was said: its event date, whole or as a day.
fn said_at(event_at: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(event_at)
        .map(|at| at.with_timezone(&chrono::Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(event_at.get(..10)?, "%Y-%m-%d")
                .ok()
                .and_then(|day| day.and_hms_opt(0, 0, 0))
                .map(|at| at.and_utc())
        })
}

/// A capture's date as a heading shows it: the day, and the time where the
/// capture has one.
fn when(event_at: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(event_at) {
        Ok(at) => at.format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => event_at.get(..10).unwrap_or(event_at).to_string(),
    }
}

/// The counter in Inception: how many signals wait for curation, from which
/// sources and how old the oldest is, with curation's chip while it reads. It
/// opens the tray (S13, S225).
pub fn counter(read: &Read) -> String {
    let waiting = waiting(read);
    let (count, sources, oldest) = summary(read, &waiting);
    let mut out = format!("<a class=\"curation\" href=\"#dock-{ID}\" data-waiting=\"{count}\">");
    match count {
        0 => out.push_str("<span>nothing waiting for curation</span>"),
        n => {
            let _ = write!(out, "<span>waiting for curation</span><b>{n}</b><span>{}</span>", escape(&sources));
            if let Some(oldest) = oldest {
                let _ = write!(out, "<span>oldest {}</span>", escape(&oldest));
            }
        }
    }
    if running(read) {
        out.push_str(&reading_chip(read));
    }
    out.push_str("</a>\n");
    out
}

/// The tray's surface in the dock (S225).
pub fn surface(read: &Read, opened: bool) -> String {
    let waiting = waiting(read);
    let (count, sources, oldest) = summary(read, &waiting);
    let mut out = format!(
        "<article class=\"surface form-tray\" id=\"dock-{ID}\" data-kind=\"tray\" data-answerable=\"false\" \
         data-opened=\"{opened}\">\n<div class=\"dk-h dk-record\">\n<div class=\"row\"><span class=\"kind\">signals</span>\
         <span class=\"ph\" data-phase=\"inception\">inception</span></div>\n<h2>Waiting for curation</h2>\n"
    );
    let tail = match (count, oldest) {
        (0, _) => "nothing waiting".to_string(),
        (n, Some(oldest)) => format!("{n} waiting · {sources} · oldest {oldest}"),
        (n, None) => format!("{n} waiting · {sources}"),
    };
    let _ = write!(out, "<div class=\"tail\">{}</div>\n", escape(&tail));
    if running(read) {
        let _ = write!(
            out,
            "<div class=\"tray-run running\"><div class=\"tray-bar\"><i></i></div>\
             <p class=\"next\">curation is reading them now {chip}</p></div>\n",
            chip = reading_chip(read),
        );
    } else if curation(read).is_some() {
        let _ = write!(
            out,
            "<div class=\"tray-run answers\"><form method=\"post\" action=\"/api/tools/curate\" class=\"answer\">\
             <button type=\"submit\" class=\"btn sm pri\" data-key=\"r\" \
             title=\"curation reads what waits now, whatever its schedule\">run curation now<span class=\"k\">r</span></button>\
             </form><span class=\"trigger\">{}</span></div>\n",
            escape(&trigger(read))
        );
    }
    out.push_str("</div>\n<div class=\"dk-b\">\n");
    // Where the operator is the curation session, its moves are made here too
    // (93b); where an agent runs it, the tray shows it working and asks nothing
    // (S225).
    let operator_runs = read.curation.as_ref().is_some_and(|session| {
        read.objects
            .iter()
            .find(|o| o.id == format!("fact/session/{session}"))
            .and_then(|fact| fact.record.get("runner").and_then(|v| v.as_str()))
            .is_none_or(|runner| runner == "operator")
    });
    if operator_runs {
        out.push_str(&curator(read));
    }
    if waiting.is_empty() {
        out.push_str(
            "<p class=\"none\">Nothing is waiting. A note you type in the box above waits here until \
             curation reads it, or until you build it, make it an intent or drop it yourself.</p>\n",
        );
    }
    out.push_str(&rows_part(read, 0));
    out.push_str("</div>\n</article>\n");
    out
}

/// The tray's rows from `from`, fifty of them, under the headings of the
/// captures they belong to, and the `more` that fetches the next fifty where
/// rows remain; a capture whose rows run past the fifty is headed again where
/// they carry on (310a, S235).
pub fn rows_part(read: &Read, from: usize) -> String {
    let waiting = waiting(read);
    let all: Vec<(usize, &signals::Signal)> = waiting
        .iter()
        .enumerate()
        .flat_map(|(at, (_, rows))| rows.iter().map(move |signal| (at, *signal)))
        .collect();
    let end = (from + lists::PAGE).min(all.len());
    let mut out = String::new();
    let mut open: Option<usize> = None;
    for (at, signal) in all.get(from.min(end)..end).unwrap_or_default() {
        if open != Some(*at) {
            if open.is_some() {
                out.push_str("</section>\n");
            }
            let (capture, rows) = &waiting[*at];
            let heard = match capture.source.as_str() {
                "offer" => format!("offered by {}", capture.captured_by),
                source => format!("from {}", source_name(source)),
            };
            let _ = write!(
                out,
                "<section class=\"sec tray-capture\" data-capture=\"{}\" data-source=\"{}\">\
                 <h3>{} · {}<span class=\"r\">{}</span></h3>\n",
                escape(&capture.capture),
                escape(&capture.source),
                escape(&heard),
                escape(&when(&capture.event_at)),
                rows.len()
            );
            open = Some(*at);
        }
        let (capture, _) = &waiting[*at];
        let said = match signal.assertion.trim().is_empty() {
            true => &signal.excerpt,
            false => &signal.assertion,
        };
        let line = match capture.source.as_str() {
            "offer" => format!("offer · offered by {}", signal.asserted_by),
            _ => [signal.kind.as_str(), signal.asserted_by.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
        };
        let _ = write!(
            out,
            // The row takes the focus as the board's note does, so twenty rows
            // read as twenty quotes and the one under the hand shows its verbs
            // (S233).
            "<div class=\"quote tray-row\" data-signal=\"{0}\" data-note=\"{0}\" tabindex=\"0\">\
             <q>{1}</q><span class=\"qm\">{2}</span>{3}</div>\n",
            escape(&signal.id),
            escape(said),
            escape(&line),
            hand::controls(read, &signal.id)
        );
    }
    if open.is_some() {
        out.push_str("</section>\n");
    }
    if end < all.len() {
        out.push_str(&lists::more(end, all.len(), Some(ID), "rows", lists::Row::Div));
    }
    out
}
