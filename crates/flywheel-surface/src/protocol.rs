//! The catalogue as a remote server of the model context protocol: how a
//! member's own client reaches the flywheel (293, 293a, 319, 320, D18).
//!
//! It is the third caller of the one catalogue, beside the machinery's
//! in-process caller and the page's HTTP routes, and it holds no operation of
//! its own: the tool listing is `CATALOGUE`, and a call is `catalogue::call`
//! under the caller's identity, recorded once as any call is (153, 193). What
//! is here is the wire alone — JSON-RPC 2.0, one message a request, carried by
//! the HTTP binding in `http.rs` — written over nothing but the store, the
//! world and the definitions a call already takes.
//!
//! The page's own views are carried here too: each read-only tool names its
//! view's resource on its declaration, the four views are listed among the
//! resources, and every address reads the page's bundle, which draws the
//! regions a result carries (293a, 322, 326, S230).
//!
//! A client is a caller and never a host: nothing here takes a lease or runs a
//! tick, and nothing a conversation around a call said is kept (324, 325).

use crate::catalogue::{self, Call, Outcome};
use crate::page::{self, VERSION, VIEWS};
use flywheel_atoms::{Received, StateStore, World};
use flywheel_domain::records::RunEntry;
use flywheel_engine::Definitions;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The versions of the protocol this server speaks, newest first. A client
/// asking for one of them is answered in it; one asking for any other is
/// answered in the newest, and decides for itself whether to go on.
pub const VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

/// Where a call over the protocol records that it came from (153).
pub const DELIVERY: &str = "client";

/// The key of a call's `_meta` naming the delivery it is, so the same call
/// delivered twice takes effect once (323, 137).
pub const DELIVERY_META: &str = "flywheel/delivery";

/// JSON-RPC's own codes, and the protocol's for a resource it does not hold.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const RESOURCE_NOT_FOUND: i64 = -32002;
/// A message the transport refused before any method was answered: the
/// caller was not admitted, or the address named another instance (253a,
/// 205a).
pub const REFUSED: i64 = -32001;

/// Who a message is answered for: the store and world a call writes through,
/// the definitions it is checked against, and the identity every response it
/// records names as given by (153, 236a, 253a).
pub struct Caller<'a, S, W: ?Sized> {
    pub store: &'a mut S,
    pub world: &'a mut W,
    pub defs: &'a Definitions,
    /// The host's address, which every link a view draws is written at (205a,
    /// 308).
    pub address: &'a str,
    pub by: &'a str,
}

/// The media type every view's resource is read with: HTML the protocol's
/// user-interface extension renders (S230, D18).
pub const VIEW_MEDIA_TYPE: &str = "text/html;profile=mcp-app";

/// What one message came to: the reply the protocol owes, where it owes one;
/// whether a call wrote anything, so the transport wakes the loop at once
/// rather than leaving it to the poll (130, D6); and every call refused, for
/// the run record (321, 79).
#[derive(Debug, Default)]
pub struct Handled {
    pub reply: Option<Value>,
    pub wrote: bool,
    pub refused: Vec<Refused>,
}

/// Who a call the transport refused before it was admitted is recorded as:
/// nobody signed in, which is all such a caller presented (253a).
pub const UNSIGNED_IN: &str = "unsigned-in";

/// A call the flywheel refused, as the run record keeps it: who asked, the
/// tool, the object it named, the delivery it came by, and why (321, 79).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub identity: String,
    pub tool: String,
    pub object: String,
    /// `client` for a member's client, `page` for the page's own controls.
    pub delivery: String,
    pub reason: String,
}

impl Refused {
    /// A call refused, named as its record would have named it: who gave it,
    /// the tool, the object it named and the delivery it came by (321, 79).
    pub fn of(call: &Call, reason: &str) -> Refused {
        Refused {
            identity: call.by.clone(),
            tool: call.tool.clone(),
            object: object_named(&call.args),
            delivery: call.delivery.clone(),
            reason: reason.to_string(),
        }
    }

    /// The run record's entry for it, on the host that refused it and at the
    /// moment it did (79).
    pub fn entry(&self, host: &str, at: chrono::DateTime<chrono::Utc>) -> RunEntry {
        RunEntry::new(at, host, "refusal", &self.object, &self.reason)
            .with("identity", &self.identity)
            .with("operation", &self.tool)
            .with("delivery", &self.delivery)
    }
}

/// Answer one message of the protocol, or a batch of them.
///
/// A request is answered with its result or its error. A notification, and a
/// response to something this server never asked, is answered with nothing,
/// as JSON-RPC says.
pub fn handle<S: StateStore, W: World + ?Sized>(
    caller: &mut Caller<'_, S, W>,
    message: &Value,
) -> Handled {
    let mut handled = Handled::default();
    let reply = match message {
        Value::Array(batch) if batch.is_empty() => Some(error(
            Value::Null,
            INVALID_REQUEST,
            "an empty batch asks nothing",
        )),
        Value::Array(batch) => {
            let replies: Vec<Value> = batch
                .iter()
                .filter_map(|one| answer(caller, one, &mut handled))
                .collect();
            (!replies.is_empty()).then(|| Value::Array(replies))
        }
        one => answer(caller, one, &mut handled),
    };
    handled.reply = reply;
    handled
}

/// The calls in a message the transport refused whole, before any was
/// answered — the caller not admitted — each as the run record keeps it (321,
/// 79, 253a).
pub fn refused_calls(message: &Value, identity: &str, reason: &str) -> Vec<Refused> {
    let one = |message: &Value| {
        (message.get("method").and_then(Value::as_str) == Some("tools/call")).then(|| {
            let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
            let arguments: BTreeMap<String, Value> = params
                .get("arguments")
                .and_then(Value::as_object)
                .map(|fields| fields.clone().into_iter().collect())
                .unwrap_or_default();
            Refused {
                identity: identity.to_string(),
                tool: params.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                object: object_named(&arguments),
                delivery: DELIVERY.to_string(),
                reason: reason.to_string(),
            }
        })
    };
    match message {
        Value::Array(batch) => batch.iter().filter_map(one).collect(),
        single => one(single).into_iter().collect(),
    }
}

/// The object a call names, as its refusal records it: whichever argument names
/// an object, a decision by its number, or that it named none (79).
fn object_named(arguments: &BTreeMap<String, Value>) -> String {
    let named = [
        "object", "decision", "session", "unit", "elaboration", "bolt", "signal", "line", "host",
        "service", "instance", "repository",
    ];
    for name in named {
        let said = match arguments.get(name) {
            Some(Value::String(said)) if !said.trim().is_empty() => said.trim().to_string(),
            Some(Value::Number(number)) => number.to_string(),
            _ => continue,
        };
        return match name {
            "decision" => format!("decision {said}"),
            _ => said,
        };
    }
    "none named".to_string()
}

/// A JSON-RPC error reply.
pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// The reply to a message the transport refused before answering it: the
/// message's own id where it carried one, so the client matches the refusal to
/// what it sent.
pub fn refused(message: &Value, reason: &str) -> Value {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    error(id, REFUSED, reason)
}

/// One message.
fn answer<S: StateStore, W: World + ?Sized>(
    caller: &mut Caller<'_, S, W>,
    message: &Value,
    handled: &mut Handled,
) -> Option<Value> {
    let Some(fields) = message
        .as_object()
        .filter(|fields| fields.get("jsonrpc") == Some(&json!("2.0")))
    else {
        return Some(error(
            Value::Null,
            INVALID_REQUEST,
            "a message is a JSON-RPC 2.0 object",
        ));
    };
    let id = fields.get("id").cloned();
    let Some(method) = fields.get("method").and_then(Value::as_str) else {
        // A response to a request this server made. It makes none, so there is
        // nothing to match it to and nothing to say (JSON-RPC 2.0).
        if fields.contains_key("result") || fields.contains_key("error") {
            return None;
        }
        return Some(error(
            id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "a message names its method",
        ));
    };
    // A notification — `notifications/initialized`, a cancellation — expects no
    // reply, and nothing it says is kept (324).
    let id = id?;
    let params = fields.get("params").cloned().unwrap_or_else(|| json!({}));
    let answered = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => call(caller, &params, handled),
        "resources/list" => Ok(json!({"resources": resources()})),
        "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
        "resources/read" => read_resource(&params),
        other => Err((METHOD_NOT_FOUND, format!("this server answers no `{other}`"))),
    };
    Some(match answered {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err((code, said)) => error(id, code, &said),
    })
}

/// What the server is and what it serves, in the version the client asked for
/// where it speaks that one.
fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .filter(|asked| VERSIONS.contains(asked))
        .unwrap_or(VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": {
            "tools": {"listChanged": false},
            "resources": {"subscribe": false, "listChanged": false},
        },
        "serverInfo": {"name": "flywheel", "title": "flywheel", "version": VERSION},
        "instructions": "Every operation of this flywheel is a tool here. `rail` shows the \
                         decisions standing now; a numbered decision is answered with `answer`, \
                         naming its number and the answer; what you noticed is kept with \
                         `capture`.",
    })
}

/// The catalogue, as the protocol declares it: the same tools and the same
/// read-only tools, in the same order, with the same arguments the in-process
/// and HTTP callers enumerate (193).
pub fn tools() -> Vec<Value> {
    catalogue::catalogue()
        .iter()
        .chain(catalogue::queries())
        .map(catalogue::Tool::declaration)
        .collect()
}

/// The page's own views, listed among the resources so a client can read one
/// before it has called any tool (322, S230). None declares an origin to fetch
/// from or asks the client's sandbox for a permission (307, 310, 204).
pub fn resources() -> Vec<Value> {
    VIEWS
        .iter()
        .map(|view| {
            json!({
                "uri": catalogue::view_address(view),
                "name": view,
                "title": match *view {
                    "rail" => "Decisions",
                    "board" => "Board",
                    "status" => "Status",
                    _ => "Detail",
                },
                "description": catalogue::query(view).map(|tool| tool.doc).unwrap_or_default(),
                "mimeType": VIEW_MEDIA_TYPE,
            })
        })
        .collect()
}

/// `resources/read`: a view's address answers the page's own bundle. An
/// address under another version is not this binary's to answer, and the
/// refusal names the addresses it serves, so a client holding an older one
/// fetches again (326, S230).
fn read_resource(params: &Value) -> Result<Value, (i64, String)> {
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or_default();
    if VIEWS.iter().any(|view| catalogue::view_address(view) == uri) {
        return Ok(json!({
            "contents": [{"uri": uri, "mimeType": VIEW_MEDIA_TYPE, "text": page::bundle()}],
        }));
    }
    let served: Vec<String> = VIEWS.iter().map(|view| catalogue::view_address(view)).collect();
    Err((
        RESOURCE_NOT_FOUND,
        match uri.starts_with("ui://flywheel/") {
            true => format!(
                "`{uri}` is not a view of this binary, which is version {VERSION}; its views are \
                 at {}",
                served.join(", ")
            ),
            false => format!("this server holds no resource `{uri}`"),
        },
    ))
}

/// A read-only tool's call: the view it names, drawn from one read, in words
/// and as the regions the view's bundle draws, with nothing recorded (193, 310,
/// 322).
fn looked<S: StateStore, W: World + ?Sized>(
    caller: &mut Caller<'_, S, W>,
    name: &str,
    arguments: Map<String, Value>,
    handled: &mut Handled,
) -> Value {
    let mut asked = Call::new(name, caller.by, DELIVERY);
    asked.args = arguments.into_iter().collect();
    match catalogue::view(caller.store, &*caller.world, caller.defs, caller.address, &asked) {
        Ok(view) => json!({
            "content": [{"type": "text", "text": view.said}],
            "structuredContent": view.handed(),
            "isError": false,
        }),
        Err(refused) => {
            handled.refused.push(Refused::of(&asked, &refused.to_string()));
            refusal(&refused.to_string())
        }
    }
}

/// A call the catalogue refused, as the protocol answers a tool that failed:
/// the reason in words, and the version it was answered under (326).
fn refusal(reason: &str) -> Value {
    json!({
        "content": [{"type": "text", "text": reason}],
        "structuredContent": {"version": VERSION, "refused": reason},
        "isError": true,
    })
}

/// `tools/call`: one call of the catalogue, under the caller's identity.
///
/// A call the catalogue refuses is the tool's error, answered as the protocol
/// answers a tool that failed, so the client can say why; it is not a fault of
/// the wire.
fn call<S: StateStore, W: World + ?Sized>(
    caller: &mut Caller<'_, S, W>,
    params: &Value,
    handled: &mut Handled,
) -> Result<Value, (i64, String)> {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err((INVALID_PARAMS, "a call names the tool it calls".into()));
    };
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(arguments)) => arguments.clone(),
        Some(_) => {
            return Err((
                INVALID_PARAMS,
                format!("`{name}` takes its arguments by name, as an object"),
            ))
        }
    };
    if catalogue::query(name).is_some() {
        return Ok(looked(caller, name, arguments, handled));
    }
    let invoked = invoked(name, arguments, params, caller.by)?;
    Ok(
        match catalogue::call(caller.store, caller.world, caller.defs, &invoked) {
            Ok(outcome) => {
                handled.wrote |= !matches!(outcome.outcome, Received::AlreadyApplied { .. });
                done(&invoked, &outcome)
            }
            // Refused: nothing is recorded as a response, and the refusal is
            // the run record's, with who asked, the tool and the object (321,
            // 79).
            Err(refused) => {
                handled.refused.push(Refused::of(&invoked, &refused.to_string()));
                refusal(&refused.to_string())
            }
        },
    )
}

/// The call a `tools/call` makes of the catalogue, under the delivery its
/// `_meta` names where it names one (323, 137).
fn invoked(name: &str, arguments: Map<String, Value>, params: &Value, by: &str) -> Result<Call, (i64, String)> {
    let mut invoked = Call::new(name, by, DELIVERY);
    invoked.args = arguments.into_iter().collect();
    match params.get("_meta").and_then(|meta| meta.get(DELIVERY_META)) {
        None | Some(Value::Null) => {}
        Some(given) => match given.as_str().and_then(delivery_id) {
            Some(id) => invoked.delivery_id = Some(id),
            None => {
                return Err((
                    INVALID_PARAMS,
                    format!(
                        "`{DELIVERY_META}` names a delivery in letters, digits, `.`, `_` and `-`, \
                         at most 64 of them and not a bare number: {given}"
                    ),
                ))
            }
        },
    }
    Ok(invoked)
}

/// The call a message makes that writes, where it is one request calling one
/// tool that is not a query: what the transport keeps for the loop's next turn
/// while a pass holds the store, or the error the message is answered with
/// where the call is malformed. Anything else waits for the store (310a, 137).
pub fn writes(message: &Value, by: &str) -> Option<Result<Call, Value>> {
    let fields = message.as_object()?;
    if fields.get("jsonrpc") != Some(&json!("2.0")) || fields.get("method").and_then(Value::as_str) != Some("tools/call") {
        return None;
    }
    let id = fields.get("id")?.clone();
    let params = fields.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params.get("name").and_then(Value::as_str)?;
    if catalogue::query(name).is_some() {
        return None;
    }
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(arguments)) => arguments.clone(),
        Some(_) => return None,
    };
    Some(invoked(name, arguments, &params, by).map_err(|(code, said)| error(id, code, &said)))
}

/// The reply to a call kept for the loop's next turn: nothing is recorded yet,
/// and the delivery it will be recorded as, once (137).
pub fn kept(message: &Value, delivery: &str, said: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": message.get("id").cloned().unwrap_or(Value::Null),
        "result": {
            "content": [{"type": "text", "text": format!("{said}; kept as {delivery}, recorded once when the host takes it up")}],
            "structuredContent": {"version": VERSION, "recorded": false, "kept": true, "delivery_id": delivery},
            "isError": false,
        },
    })
}

/// The id a delivery a client names is recorded under: `client-` and the name.
///
/// The name is letters, digits, `.`, `_` and `-`, at most 64 of them, and not a
/// bare number, which is what a call naming no delivery is counted by (137).
pub fn delivery_id(given: &str) -> Option<String> {
    let fits = !given.is_empty()
        && given.len() <= 64
        && given
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !given.chars().all(|c| c.is_ascii_digit());
    fits.then(|| format!("{DELIVERY}-{given}"))
}

/// A call's result: what was recorded, in words for a client that reads, and
/// the same fields the HTTP caller answers with for one that does not (193).
fn done(invoked: &Call, outcome: &Outcome) -> Value {
    let mut said = vec![match &outcome.outcome {
        Received::Recorded { .. } => format!("recorded {}, given by {}", outcome.id, invoked.by),
        Received::AlreadyApplied { .. } => format!(
            "delivered before as {}; nothing was written again (137)",
            outcome.id
        ),
        Received::Unapplicable { reason, .. } => {
            format!("recorded {}, and it cannot apply: {reason}", outcome.id)
        }
    }];
    said.extend(
        outcome
            .journal
            .iter()
            .filter(|noted| noted.kind != "create")
            .map(|noted| format!("{} {}: {}", noted.kind, noted.object, noted.text)),
    );
    let mut structured = json!({
        "version": VERSION,
        "id": outcome.id,
        "recorded": catalogue::recorded(outcome),
    });
    if let Some(ask) = catalogue::asked(outcome) {
        structured["ask"] = json!(ask);
    }
    json!({
        "content": [{"type": "text", "text": said.join("\n")}],
        "structuredContent": structured,
        "isError": false,
    })
}
