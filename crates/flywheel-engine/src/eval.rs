//! Guard evaluation over a snapshot. Pure.

use crate::defs::{Definitions, Guard, State};
use crate::runtime::{Object, ResponseKind, Snapshot};
use chrono::Duration;
use serde_json::Value;

/// What a guard needs to know about where it is evaluated.
pub struct Ctx<'a> {
    pub defs: &'a Definitions,
    pub snap: &'a Snapshot<'a>,
    pub object: &'a Object,
    /// The region path of the transition being considered.
    pub region: &'a str,
    /// The state the transition leaves.
    pub state_name: &'a str,
    pub state: &'a State,
}

/// The outcome of a guard: whether it holds, and the response it consumed (id, bound argument).
#[derive(Debug, Default, Clone)]
pub struct Hold {
    pub holds: bool,
    pub response: Option<(String, String)>,
}

impl Hold {
    fn yes() -> Self { Hold { holds: true, response: None } }
    fn no() -> Self { Hold { holds: false, response: None } }
}

pub fn eval(g: &Guard, cx: &Ctx) -> Hold {
    match g {
        Guard::All { all } => {
            let mut resp = None;
            for x in all {
                let h = eval(x, cx);
                if !h.holds { return Hold::no(); }
                if h.response.is_some() { resp = h.response; }
            }
            Hold { holds: true, response: resp }
        }
        Guard::Any { any } => {
            for x in any {
                let h = eval(x, cx);
                if h.holds { return h; }
            }
            Hold::no()
        }
        Guard::Not { not } => {
            if eval(not, cx).holds { Hold::no() } else { Hold::yes() }
        }
        Guard::Always { always } => if *always { Hold::yes() } else { Hold::no() },
        Guard::Ev(e) => if eval_ev(e, cx) { Hold::yes() } else { Hold::no() },
        Guard::Response { response } => eval_response(response, cx),
        Guard::Final { final_state } => {
            // `final: X` (or `final: <region>.X`): the state's direct submachine regions, or an inline
            // nested region whose state runs a submachine, is in state X. Deeper nesting is not seen.
            let prefix = format!("{}.{}.", cx.region, cx.state_name);
            let (want_region, want_state) = match final_state.split_once('.') { Some((r, s)) => (Some(r), s), None => (None, final_state.as_str()) };
            let hit = cx.object.config.iter().any(|(path, st)| {
                // `final: <region>.X` may also name a sibling region of the object (a bolt's `line`).
                if let Some(r) = want_region {
                    if st == want_state && path.split('.').step_by(2).any(|seg| seg == r) { return true; }
                }
                let Some(rest) = path.strip_prefix(&prefix) else { return false };
                let parts: Vec<&str> = rest.split('.').collect();
                let visible = match parts.len() {
                    1 => true,
                    3 => cx.state.regions.contains_key(parts[0]),
                    _ => false,
                };
                if !visible { return false; }
                let region_name = parts.last().copied().unwrap_or("");
                want_region.map(|r| r == region_name || r == parts[0]).unwrap_or(true) && st == want_state
            });
            if hit { Hold::yes() } else { Hold::no() }
        }
        Guard::Children { children } => {
            let kids: Vec<&Object> = cx
                .snap
                .objects
                .values()
                .filter(|o| o.parent.as_deref() == Some(&cx.object.id) && (o.machine == children.kind || machine_object(cx, &o.machine) == children.kind))
                .collect();
            let states: Vec<&str> = kids.iter().filter_map(|k| k.top_state()).collect();
            let mut ok = true;
            if let Some(all) = &children.all {
                ok &= !states.is_empty() && states.iter().all(|s| all.iter().any(|a| a == s));
            }
            if let Some(any) = &children.any {
                ok &= states.iter().any(|s| any.iter().any(|a| a == s));
            }
            if let Some(none) = &children.none {
                ok &= !states.iter().any(|s| none.iter().any(|a| a == s));
            }
            if let Some(n) = children.count_gte {
                ok &= kids.len() >= n;
            }
            if ok { Hold::yes() } else { Hold::no() }
        }
        Guard::Parent { parent } => {
            let Some(pid) = &cx.object.parent else { return Hold::no() };
            let Some(p) = cx.snap.objects.get(pid) else { return Hold::no() };
            let hit = p.top_states().iter().any(|s| parent.in_.iter().any(|x| x == s));
            if hit { Hold::yes() } else { Hold::no() }
        }
        Guard::Region { region } => {
            // A dotted name is a path suffix (`place.place.life`); a bare name matches a region of the
            // state's own submachine or nested regions (descendants), else a sibling region.
            if region.name.contains('.') {
                let suffix = format!(".{}", region.name);
                let hit = cx.object.config.iter().any(|(path, st)| (path == &region.name || path.ends_with(&suffix)) && region.in_.iter().any(|x| x == st));
                return if hit { Hold::yes() } else { Hold::no() };
            }
            let below = format!("{}.{}.", cx.region, cx.state_name);
            let parent_path = cx.region.rsplit_once('.').map(|(p, _)| p.to_string());
            let hit = cx.object.config.iter().any(|(path, st)| {
                let last = path.rsplit('.').next().unwrap_or("");
                if last != region.name { return false; }
                let is_desc = path.starts_with(&below);
                let pp = path.rsplit_once('.').map(|(p, _)| p.to_string());
                (is_desc || pp == parent_path) && region.in_.iter().any(|x| x == st)
            });
            if hit { Hold::yes() } else { Hold::no() }
        }
    }
}

fn machine_object(cx: &Ctx, machine: &str) -> String {
    cx.defs.get(machine).and_then(|m| m.object.clone()).unwrap_or_else(|| machine.to_string())
}

/// Engine-provided evidence first, then the control plane's.
pub fn evidence(cx: &Ctx, name: &str) -> Option<Value> {
    match name {
        "state" => cx.object.config.get(cx.region).map(|s| Value::String(s.clone())),
        "entered_at" => cx.object.entered_at.get(cx.region).map(|t| Value::String(t.to_rfc3339())),
        "seq" => Some(Value::from(cx.object.seq)),
        "applied_responses" => Some(Value::Array(cx.object.applied_responses.iter().map(|s| Value::String(s.clone())).collect())),
        "now" => Some(Value::String(cx.snap.now.to_rfc3339())),
        _ => {
            if let Some(v) = cx.snap.evidence.evidence(&cx.object.id, cx.region, name) {
                return Some(v);
            }
            // A record field read as evidence, so that a machine may test its own record.
            cx.object.record.get(name).cloned()
        }
    }
}

fn eval_ev(e: &crate::defs::EvGuard, cx: &Ctx) -> bool {
    let v = evidence(cx, &e.ev);
    if let Some(exists) = e.exists {
        return v.is_some() == exists;
    }
    if let Some(older) = &e.older {
        let Some(Value::String(ts)) = &v else { return false };
        let Ok(t) = chrono::DateTime::parse_from_rfc3339(ts) else { return false };
        let Some(d) = parse_duration(older) else { return false };
        return cx.snap.now.signed_duration_since(t.with_timezone(&chrono::Utc)) > d;
    }
    let Some(v) = v else { return false };
    if let Some(is) = &e.is {
        return json_eq(&v, is);
    }
    if let Some(set) = &e.in_ {
        return set.iter().any(|x| json_eq(&v, x));
    }
    if let Some(x) = &e.gt { return cmp(&v, x) == Some(std::cmp::Ordering::Greater); }
    if let Some(x) = &e.gte { return matches!(cmp(&v, x), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)); }
    if let Some(x) = &e.lt { return cmp(&v, x) == Some(std::cmp::Ordering::Less); }
    if let Some(x) = &e.lte { return matches!(cmp(&v, x), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)); }
    let other = |n: &String| evidence(cx, n);
    if let Some(n) = &e.eq_ev { return other(n).map(|o| json_eq(&v, &o)).unwrap_or(false); }
    if let Some(n) = &e.ne_ev { return other(n).map(|o| !json_eq(&v, &o)).unwrap_or(true); }
    if let Some(n) = &e.gte_ev { return other(n).map(|o| matches!(cmp(&v, &o), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal))).unwrap_or(false); }
    if let Some(n) = &e.lt_ev { return other(n).map(|o| cmp(&v, &o) == Some(std::cmp::Ordering::Less)).unwrap_or(false); }
    if let Some(n) = &e.gt_ev { return other(n).map(|o| cmp(&v, &o) == Some(std::cmp::Ordering::Greater)).unwrap_or(false); }
    // `{ev: x}` alone: truthy.
    truthy(&v)
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty() && s != "none" && s != "absent" && s != "false",
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(x), Value::Bool(y)) => (x == "true") == *y,
        (Value::Bool(x), Value::String(y)) => (y == "true") == *x,
        (Value::Number(x), Value::String(y)) => x.to_string() == *y,
        (Value::String(x), Value::Number(y)) => *x == y.to_string(),
        _ => a == b,
    }
}

fn cmp(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x.partial_cmp(&y),
        _ => match (a.as_str(), b.as_str()) {
            (Some(x), Some(y)) => Some(x.cmp(y)),
            _ => None,
        },
    }
}

/// `5m`, `14h`, `7d`, `30s`.
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    let (num, unit) = s.split_at(s.trim_end_matches(|c: char| c.is_ascii_alphabetic()).len());
    let n: i64 = num.trim().parse().ok()?;
    Some(match unit {
        "s" => Duration::seconds(n),
        "m" => Duration::minutes(n),
        "h" => Duration::hours(n),
        "d" => Duration::days(n),
        "w" => Duration::weeks(n),
        _ => return None,
    })
}

/// `{response: "yes"}`, `{response: "bolt <name>"}`, `{response: "redo: <notes>"}`.
/// Holds when an unapplied response answers this object's active decision at this
/// state, or is a dictation naming this object, and its answer matches the pattern.
fn eval_response(pattern: &str, cx: &Ctx) -> Hold {
    // The decision a response answers may sit on this state or on any active nested state of the
    // object (a bolt's close decision is on its `close` region; the transition is on `open`).
    let mut kinds: Vec<String> = cx.state.decision.as_ref().map(|d| vec![d.kind.clone()]).unwrap_or_default();
    for region in cx.object.config.keys() {
        if let Some((_, st)) = crate::tick::state_def(cx.defs, cx.object, region) {
            if let Some(d) = &st.decision { if !kinds.contains(&d.kind) { kinds.push(d.kind.clone()); } }
        }
    }
    for r in cx.snap.responses {
        if cx.object.applied_responses.iter().any(|a| a == &r.id) {
            continue;
        }
        let addressed = match r.kind {
            ResponseKind::Answer => {
                let Some(n) = r.decision else { continue };
                let Some(did) = cx.snap.register.decision_of(n) else { continue };
                // decision id = <object>/<kind>/<entered_at>
                let mut parts = did.rsplitn(3, '/');
                let _since = parts.next();
                let kind = parts.next();
                let obj = parts.next();
                obj == Some(cx.object.id.as_str()) && kind.map(|k| kinds.iter().any(|x| x == k)).unwrap_or(false)
            }
            ResponseKind::Dictation => r.object.as_deref() == Some(cx.object.id.as_str()),
        };
        if !addressed {
            continue;
        }
        if let Some(arg) = match_answer(pattern, &r.answer) {
            return Hold { holds: true, response: Some((r.id.clone(), arg)) };
        }
    }
    Hold::no()
}

/// Match an answer against a pattern: a bare word, `word <arg>`, or `word: <text>`.
/// Returns the bound argument (empty for a bare word).
pub fn match_answer(pattern: &str, answer: &str) -> Option<String> {
    let p = pattern.trim();
    let a = answer.trim();
    if let Some(head) = p.strip_suffix('>').and_then(|s| s.rsplit_once('<')).map(|(h, _)| h.trim_end()) {
        // `bolt <name>` → head "bolt"; `redo: <notes>` → head "redo:"
        let head_word = head.trim_end_matches(':').trim();
        let colon = head.ends_with(':');
        // The head may be several words (`new bolt <name>`) or none (`<text>`).
        let rest = a.strip_prefix(head_word)?;
        if colon {
            return Some(rest.trim_start().strip_prefix(':')?.trim().to_string());
        }
        if !head_word.is_empty() && !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return None;
        }
        return Some(rest.trim().to_string());
    }
    if a == p {
        return Some(String::new());
    }
    None
}
