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
//! A client is a caller and never a host: nothing here takes a lease or runs a
//! tick, and nothing a conversation around a call said is kept (324, 325).

use crate::catalogue::{self, Call, Outcome};
use flywheel_atoms::{Received, StateStore, World};
use flywheel_engine::Definitions;
use serde_json::{json, Map, Value};

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
    pub by: &'a str,
}

/// What one message came to: the reply the protocol owes, where it owes one,
/// and whether a call wrote anything, so the transport wakes the loop at once
/// rather than leaving it to the poll (130, D6).
#[derive(Debug, Default)]
pub struct Handled {
    pub reply: Option<Value>,
    pub wrote: bool,
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
    handled.reply = match message {
        Value::Array(batch) if batch.is_empty() => Some(error(
            Value::Null,
            INVALID_REQUEST,
            "an empty batch asks nothing",
        )),
        Value::Array(batch) => {
            let replies: Vec<Value> = batch
                .iter()
                .filter_map(|one| answer(caller, one, &mut handled.wrote))
                .collect();
            (!replies.is_empty()).then(|| Value::Array(replies))
        }
        one => answer(caller, one, &mut handled.wrote),
    };
    handled
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
    wrote: &mut bool,
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
        "tools/call" => call(caller, &params, wrote),
        "resources/list" => Ok(json!({"resources": []})),
        "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
        "resources/read" => Err((
            RESOURCE_NOT_FOUND,
            format!(
                "this server holds no resource `{}`",
                params.get("uri").and_then(Value::as_str).unwrap_or_default()
            ),
        )),
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
        "serverInfo": {"name": "flywheel", "title": "flywheel", "version": crate::page::VERSION},
        "instructions": "Every operation of this flywheel is a tool here. A numbered decision \
                         is answered with `answer`, naming its number and the answer; what you \
                         noticed is kept with `capture`.",
    })
}

/// The catalogue, as the protocol declares it: the same tools, in the same
/// order, with the same arguments the in-process and HTTP callers enumerate
/// (193).
pub fn tools() -> Vec<Value> {
    catalogue::catalogue()
        .iter()
        .map(catalogue::Tool::declaration)
        .collect()
}

/// `tools/call`: one call of the catalogue, under the caller's identity.
///
/// A call the catalogue refuses is the tool's error, answered as the protocol
/// answers a tool that failed, so the client can say why; it is not a fault of
/// the wire.
fn call<S: StateStore, W: World + ?Sized>(
    caller: &mut Caller<'_, S, W>,
    params: &Value,
    wrote: &mut bool,
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
    let mut invoked = Call::new(name, caller.by, DELIVERY);
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
    Ok(
        match catalogue::call(caller.store, caller.world, caller.defs, &invoked) {
            Ok(outcome) => {
                *wrote |= !matches!(outcome.outcome, Received::AlreadyApplied { .. });
                done(&invoked, &outcome)
            }
            Err(refused) => json!({
                "content": [{"type": "text", "text": refused.to_string()}],
                "structuredContent": {"refused": refused.to_string()},
                "isError": true,
            }),
        },
    )
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
    let mut structured = json!({"id": outcome.id, "recorded": catalogue::recorded(outcome)});
    if let Some(ask) = catalogue::asked(outcome) {
        structured["ask"] = json!(ask);
    }
    json!({
        "content": [{"type": "text", "text": said.join("\n")}],
        "structuredContent": structured,
        "isError": false,
    })
}
