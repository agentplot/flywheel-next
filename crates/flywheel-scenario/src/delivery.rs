//! What a scripted curation session delivered, read out of its commits.
//!
//! A real curation session delivers records: one move per signal it judged and
//! the proposed intents its joins produce (`curation.yaml` applying). The
//! stand-in plays a session from a script, and a script says in one line what
//! the session committed in its place — so this reads that line into the same
//! records a real session would have written.
//!
//! It lives beside the runner and not in the binary, for the reason the
//! dictation interpreter does: nothing that ships reads prose (194). What the
//! machinery does with the records is `flywheel-domain`'s, and is the same
//! whoever produced them (20).

use flywheel_domain::signals::{Delivered, Move, Proposal};

/// Read a curation session's delivery.
///
/// The line names counts and targets:
/// `moves: 6 attach intent/a, 9 drop, 5 join (3 -> intent/b challenges c@3,
/// 2 -> intent/d)` — six attached to that intent, nine dropped, five joined
/// across two proposed intents, one of which challenges a claim (S08).
///
/// The signals are numbered under the capture the run read, because a script
/// says how many were judged rather than naming each: that is the stand-in
/// filling in what a real session would have named (93).
pub fn curation(commits: &[String], key: &str, at: &str) -> Delivered {
    let mut delivered = Delivered::default();
    let mut next = 0u64;
    let mut signal = |delivered: &Delivered| {
        let _ = delivered;
        next += 1;
        flywheel_domain::signals::signal_object(key, next)
    };
    for line in commits {
        let Some(body) = line.split_once("moves:").map(|(_, rest)| rest.trim()) else {
            continue;
        };
        for clause in split_clauses(body) {
            let (head, joins) = match clause.split_once('(') {
                Some((head, rest)) => (head.trim(), rest.trim_end_matches(')')),
                None => (clause.as_str(), ""),
            };
            let words: Vec<&str> = head.split_whitespace().collect();
            let [count, word, rest @ ..] = words.as_slice() else {
                continue;
            };
            let Ok(count) = count.parse::<usize>() else {
                continue;
            };
            let target = rest.first().copied().unwrap_or_default();
            match *word {
                // Every join names the proposed intent it joins, so the joins
                // are read from the clause in brackets and the count is their
                // sum (109, 116).
                "join" => {
                    for (n, into, challenges) in proposed(joins) {
                        let mut proposal = Proposal {
                            id: into.clone(),
                            challenges,
                            ..Default::default()
                        };
                        for _ in 0..n {
                            let id = signal(&delivered);
                            proposal.signals.push(id.clone());
                            delivered.moves.push(Move {
                                signal: id,
                                target: format!("join {into}"),
                                reason: "curation clustered it".into(),
                                at: at.to_string(),
                            });
                        }
                        delivered.proposals.push(proposal);
                    }
                }
                other => {
                    for _ in 0..count {
                        let id = signal(&delivered);
                        delivered.moves.push(Move {
                            signal: id,
                            target: match target.is_empty() {
                                true => other.to_string(),
                                false => format!("{other} {target}"),
                            },
                            reason: "curation judged it".into(),
                            at: at.to_string(),
                        });
                    }
                }
            }
        }
    }
    delivered
}

/// The clauses of the line, splitting on commas that are not inside brackets.
fn split_clauses(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut held = String::new();
    for c in body.chars() {
        match c {
            '(' => {
                depth += 1;
                held.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                held.push(c);
            }
            ',' if depth == 0 => {
                out.push(held.trim().to_string());
                held.clear();
            }
            _ => held.push(c),
        }
    }
    if !held.trim().is_empty() {
        out.push(held.trim().to_string());
    }
    out
}

/// The proposed intents a join clause names: `3 -> intent/b challenges c@3,
/// 2 -> intent/d`.
fn proposed(joins: &str) -> Vec<(usize, String, Vec<String>)> {
    let mut out = Vec::new();
    for part in joins.split(',') {
        let Some((count, rest)) = part.trim().split_once("->") else {
            continue;
        };
        let Ok(count) = count.trim().parse::<usize>() else {
            continue;
        };
        let words: Vec<&str> = rest.split_whitespace().collect();
        let Some(into) = words.first() else { continue };
        let challenges = words
            .iter()
            .skip_while(|w| **w != "challenges")
            .skip(1)
            .map(|w| w.to_string())
            .collect();
        out.push((count, into.to_string(), challenges));
    }
    out
}
