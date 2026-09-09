//! `flywheel host run --driven`: a host whose clock and whose sweep come from
//! the caller rather than from a timer (D15, 232).
//!
//! A scenario's clock is virtual and moves for exactly two reasons — a `clock`
//! step, and each `tick` step by one interval — and wall-clock time never
//! reaches a guard. A host that is a process of its own must take that same
//! clock, or a two-host run would be a race with the wall (D15, D7). So the
//! process reads one command per line and answers one line per command: the
//! caller says what time it is and when to sweep, and nothing here keeps time.
//!
//! Losing a host is not a command: the caller stops the process without a
//! farewell, which is what makes the heartbeat stop and the lease go stale
//! (S13, 150).

use crate::host::Host;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::io::{BufRead, Write};

/// What the process says when it is open and has read nothing yet.
pub const READY: &str = "ready";

/// Read commands until the stream ends or `quit` arrives. Every answer is one
/// line: `ok …` or `err …`. A problem with the machinery is reported and the
/// loop goes on; it is never work and never a crash (81).
pub fn drive(host: &mut Host, input: impl BufRead, mut output: impl Write) -> Result<()> {
    writeln!(output, "{READY} {}", host.name)?;
    output.flush()?;
    for line in input.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
        if verb == "quit" {
            writeln!(output, "ok bye")?;
            output.flush()?;
            return Ok(());
        }
        match act(host, verb, rest.trim()) {
            Ok(said) => writeln!(output, "ok {said}")?,
            Err(e) => {
                host.report_problem(&format!("host/{}", host.name), &format!("{e:#}"));
                writeln!(output, "err {}", format!("{e:#}").replace('\n', " · "))?
            }
        }
        output.flush()?;
    }
    Ok(())
}

/// One command. The set is closed: a word this does not name is refused rather
/// than ignored, so a typo in the caller is not a silent no-op.
fn act(host: &mut Host, verb: &str, rest: &str) -> Result<String> {
    match verb {
        // The injected clock: every timed behaviour of this host is a guard on
        // the tick, and this is the only clock there is (D7, 231, D15).
        "clock" => {
            let at = DateTime::parse_from_rfc3339(rest)
                .map_err(|_| anyhow::anyhow!("`{rest}` is no RFC 3339 time"))?
                .with_timezone(&Utc);
            host.set_now(at);
            Ok(format!("clock {}", at.to_rfc3339()))
        }
        // This host's declaration and its heartbeat: what it takes leases
        // within, and that it is alive (147, 149, 163). A host writes it as it
        // starts, before it has ticked anything.
        "declare" => {
            host.declare()?;
            Ok(format!("declared {}", host.name))
        }
        // What the world reports, told to this host directly: a session on this
        // machine finishing is a local fact, and a host whose route is cut
        // still learns it (B.3, 151).
        "given" => {
            let mut parts = rest.splitn(3, '\t');
            let (Some(object), Some(name), Some(value)) =
                (parts.next(), parts.next(), parts.next())
            else {
                anyhow::bail!("`given` takes <object>\\t<name>\\t<json>");
            };
            let value: serde_json::Value = serde_json::from_str(value)
                .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));
            host.store.git.set_given(object, name, value);
            Ok(format!("given {object} {name}"))
        }
        // The sweep, triggered by the caller rather than by a timer: every
        // scope this host has a lease or a candidate on (D7, 130).
        "sweep" => Ok(format!("swept {}", host.sweep()?)),
        // One pass: the notify-tick for what moved, then the sweep if it is due.
        "once" => Ok(format!("fired {}", host.once()?)),
        // The route is cut: keep ticking what is held, commit locally, take no
        // lease, start nothing new, deliver to no sink (151, D4a).
        "disconnect" => {
            host.store.git.disconnected = true;
            Ok("disconnected".into())
        }
        // Back: renewals push first, then the commits made while away (165,
        // D4a). A renewal a takeover rejected is how this host learns it lost.
        "reconnect" => {
            host.store.git.disconnected = false;
            host.store.git.fetch()?;
            let renewed = host.renew_held()?;
            let landed = host.store.git.push_unpushed()?;
            Ok(format!("reconnected renewals {renewed} landed {landed}"))
        }
        // Away with since-when: the leases stand and the stall clocks pause
        // (150a).
        "away" => {
            let since = host.now();
            host.go_away(since);
            Ok(format!("away since {}", since.to_rfc3339()))
        }
        "back" => {
            host.come_back()?;
            Ok("back".into())
        }
        // The rail as this host derives it: the standing decisions with the
        // numbers the register gave them, in one line. Derived from the active
        // states and the register on every read and stored nowhere, so two
        // hosts of one instance answer alike or one of them is wrong (7, 15,
        // 147, 148).
        "rail" => {
            let defs = host.defs.clone();
            let mut lines: Vec<String> = crate::console::rail(&mut host.store, &defs)?
                .iter()
                .map(|d| {
                    format!(
                        "{}={}/{}",
                        d.number.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
                        d.object,
                        d.kind
                    )
                })
                .collect();
            lines.sort();
            Ok(format!("rail {}", lines.join(" ")))
        }
        // What this host reads, so a caller can prove it read rather than
        // guessed. The state itself is read from the store by the caller.
        "state" => Ok(format!("at {}", host.store.git.fetched)),
        other => anyhow::bail!(
            "`{other}` is no command; they are clock, declare, given, sweep, once, \
             disconnect, reconnect, away, back, rail, state, quit"
        ),
    }
}
