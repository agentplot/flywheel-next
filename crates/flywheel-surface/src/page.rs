//! The served page: one bundle that is the rail, the capture box and the
//! status view (D11, `surfaces/page`).
//!
//! It is rendered from the register and the objects on every request. No
//! rendering is stored (15), the page holds no client state a reload loses
//! (310), and the bundle fetches nothing from anywhere else — every style and
//! every script it needs is in the document it serves (310).
//!
//! Under 760px it is the same bundle: two tabs, Decisions and Board, with the
//! dock full screen and a back control (307). One bundle is built and one is
//! served, and its version is the binary's (307).

use crate::links;
use flywheel_atoms::StateStore;
use flywheel_domain::{commands, status};
use flywheel_domain::sinks;
use flywheel_engine::{DecisionInstance, Definitions, Object};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The version the bundle carries: the binary's own (307).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What one request read. Everything the page shows comes from here, so the
/// whole page is one read and a reload shows what is recorded (310).
pub struct Read {
    pub decisions: Vec<DecisionInstance>,
    pub status: status::Status,
    pub objects: Vec<Object>,
    /// The host's address, for the links every rail line carries (308, 205a).
    pub address: String,
    /// The identity every response records as given by (153, 236a, 253a).
    pub operator: String,
    /// The objects held by a host past its stale window: a link to one says the
    /// host is away and since when, rather than failing silently (308, 150a).
    pub away: BTreeMap<String, sinks::Away>,
}

/// Read once, for one request (310).
pub fn read<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    address: &str,
    operator: &str,
) -> anyhow::Result<Read> {
    let decisions = commands::rail(store, defs)?;
    let at = commands::now(store)?;
    let as_of = store.read(flywheel_domain::RAIL)?.as_of;
    let status = status::read(
        store,
        defs,
        &as_of,
        at,
        chrono::Duration::minutes(5),
        chrono::Duration::minutes(30),
    )?;
    let objects = store.list_records(&flywheel_atoms::Scope::All)?;
    let away = sinks::away_by_object(store, at, chrono::Duration::minutes(5))?;
    Ok(Read {
        decisions,
        status,
        objects,
        address: address.to_string(),
        operator: operator.to_string(),
        away,
    })
}

/// The kinds the page gives a form of its own. The phase an object is in is
/// shown by where it sits and never by its form (209).
pub const KINDS: [&str; 6] = [
    "decision",
    "intent",
    "elaboration",
    "bolt",
    "proposal",
    "signal",
];

/// Render the whole page. One document, one request, nothing stored.
pub fn render(read: &Read) -> String {
    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = write!(out, "<meta name=\"flywheel-version\" content=\"{VERSION}\">\n");
    out.push_str("<title>flywheel</title>\n");
    out.push_str(&style());
    out.push_str("</head>\n<body>\n");
    let _ = write!(
        out,
        "<header><span class=\"instance\">{}</span> <span class=\"given-by\">{}</span></header>\n",
        escape(read.address.rsplit('/').next().unwrap_or_default()),
        escape(&read.operator)
    );
    out.push_str("<nav class=\"tabs\" role=\"tablist\">\n");
    out.push_str("<a class=\"tab\" id=\"tab-decisions\" href=\"#decisions\" role=\"tab\">Decisions</a>\n");
    out.push_str("<a class=\"tab\" id=\"tab-board\" href=\"#board\" role=\"tab\">Board</a>\n");
    out.push_str("</nav>\n");
    out.push_str(&decisions(read));
    out.push_str(&capture_box());
    out.push_str(&board(read));
    out.push_str(&dock(read));
    out.push_str("</body>\n</html>\n");
    out
}

/// The rail: every standing decision with its number and its answers, one tap
/// each (15, 311).
fn decisions(read: &Read) -> String {
    let mut out = String::from("<section id=\"decisions\" class=\"tab-panel\">\n");
    if read.decisions.is_empty() {
        out.push_str("<p class=\"none\">nothing to decide</p>\n");
    }
    for decision in &read.decisions {
        let number = decision.number.map(|n| n.to_string()).unwrap_or_default();
        let link = links::to_object(&read.address, &decision.object).unwrap_or_default();
        let _ = write!(
            out,
            "<article class=\"card decision\" data-kind=\"decision\" data-number=\"{number}\" \
             data-object=\"{}\" data-group=\"{}\">\n",
            escape(&decision.object),
            escape(&decision.group)
        );
        let _ = write!(out, "<span class=\"number\">{number}</span>\n");
        let _ = write!(
            out,
            "<a class=\"object\" href=\"{}\">{}</a>\n",
            escape(&link),
            escape(&decision.object)
        );
        if let Some(away) = read.away.get(&decision.object) {
            let _ = write!(
                out,
                "<p class=\"away\" data-away-host=\"{}\">{}</p>\n",
                escape(&away.host),
                escape(&away.said())
            );
        }
        out.push_str("<div class=\"answers\">\n");
        for answer in &decision.answers {
            // One tap each, and nothing behind a hover or a keyboard (311).
            let _ = write!(
                out,
                "<form method=\"post\" action=\"/api/tools/answer\" class=\"answer\">\n\
                 <input type=\"hidden\" name=\"decision\" value=\"{number}\">\n\
                 <input type=\"hidden\" name=\"answer\" value=\"{0}\">\n\
                 <button type=\"submit\" data-answer=\"{0}\">{0}</button>\n</form>\n",
                escape(answer)
            );
        }
        out.push_str("</div>\n</article>\n");
    }
    out.push_str("</section>\n");
    out
}

/// The page's one typed input: text is a capture with one signal of kind ask,
/// unparsed, and marking it an intent is a control beside the field and never a
/// word read out of the text (19, 194).
fn capture_box() -> String {
    let mut out = String::from("<section id=\"capture\" class=\"capture\">\n");
    out.push_str("<form method=\"post\" action=\"/api/tools/capture\" id=\"capture-box\">\n");
    out.push_str(
        "<label for=\"capture-text\">Capture</label>\n\
         <textarea id=\"capture-text\" name=\"text\" rows=\"2\"></textarea>\n",
    );
    // The judgment is the control's, made after the text is captured whole.
    out.push_str(
        "<label class=\"mark-intent\"><input type=\"checkbox\" id=\"mark-intent\" \
         name=\"intent\" value=\"yes\"> Mark as intent</label>\n",
    );
    out.push_str("<button type=\"submit\">Capture</button>\n</form>\n</section>\n");
    out
}

/// The status view, from the same read (141, 310).
fn board(read: &Read) -> String {
    let mut out = String::from("<section id=\"board\" class=\"tab-panel\">\n");
    let rendered = status::render(&read.status);
    // The projection's own body, minus the document wrapper it carries for the
    // committed file: the page is one document (132, D12).
    out.push_str(&body_of(&rendered.body));
    out.push_str("</section>\n");
    out
}

/// The dock: one surface per object, each kind in its own form, full screen
/// under 760px with a back control (209, 210, 307).
fn dock(read: &Read) -> String {
    let mut out = String::from("<section id=\"dock\" class=\"dock\">\n");
    out.push_str("<a class=\"back\" id=\"dock-back\" href=\"#board\">Back</a>\n");
    for object in &read.objects {
        let kind = match object.machine.as_str() {
            m if KINDS.contains(&m) => m,
            other => other,
        };
        let link = links::to_object(&read.address, &object.id).unwrap_or_default();
        let _ = write!(
            out,
            "<article class=\"surface form-{kind}\" id=\"dock-{}\" data-kind=\"{kind}\" \
             data-answerable=\"false\">\n",
            escape(&object.id)
        );
        let _ = write!(
            out,
            "<a class=\"object\" href=\"{}\">{}</a>\n",
            escape(&link),
            escape(&object.id)
        );
        // A link to a host past its stale window opens this surface and says
        // the host is away and since when, rather than failing silently
        // (308, 150a).
        if let Some(away) = read.away.get(&object.id) {
            let _ = write!(
                out,
                "<p class=\"away\" data-away-host=\"{}\">{}</p>\n",
                escape(&away.host),
                escape(&away.said())
            );
        }
        // An elaboration is a surface of its own, reached from its intent, and
        // an intent lists its elaborations in order (210).
        if object.machine == "intent" {
            out.push_str("<ol class=\"elaborations\">\n");
            for child in read
                .objects
                .iter()
                .filter(|o| o.machine == "elaboration" && o.parent.as_deref() == Some(&object.id))
            {
                let _ = write!(
                    out,
                    "<li><a class=\"elaboration\" href=\"#dock-{0}\">{0}</a></li>\n",
                    escape(&child.id)
                );
            }
            out.push_str("</ol>\n");
        }
        out.push_str("</article>\n");
    }
    out.push_str("</section>\n");
    out
}

/// The body of a rendered document, without its wrapper.
fn body_of(document: &str) -> String {
    match (document.find("<body>"), document.find("</body>")) {
        (Some(open), Some(close)) if close > open => document[open + 6..close].to_string(),
        _ => document.to_string(),
    }
}

/// The one stylesheet, in the document. Under 760px the same bundle lays out as
/// two tabs with the dock full screen (307, 310).
fn style() -> String {
    String::from(
        "<style>\n\
         body { font: 16px/1.5 system-ui, sans-serif; margin: 0; }\n\
         .tabs { display: none; }\n\
         .card, .surface { padding: .75rem; border-bottom: 1px solid #ddd; }\n\
         .answers button { min-height: 44px; min-width: 44px; }\n\
         .dock { position: static; }\n\
         .back { display: none; }\n\
         @media (max-width: 760px) {\n\
         .tabs { display: flex; }\n\
         .tab-panel { display: none; }\n\
         .tab-panel:target, #decisions:not(:target) { display: block; }\n\
         .dock { position: fixed; inset: 0; background: #fff; overflow: auto; }\n\
         .back { display: block; }\n\
         }\n\
         </style>\n",
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
