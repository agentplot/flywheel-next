//! What a decision asks, in a sentence, and what each answer is called on its
//! control (S220).
//!
//! The model names a decision by its kind and its answers by the words the
//! response grammar matches (`yes`, `hold`, `build`). A card that printed
//! those alone — "bolt-close · yes · hold" — told the operator which machine
//! was asking and not what they were being asked. The sentence is the
//! question; the label is what pressing the control does; the key is how it
//! is pressed without the mouse. The answer posted is still the model's own
//! word, so the chat's numbered grammar and the page share one write path
//! (193, 194).

/// The question a decision of this kind puts, with the object's name and what
/// the page knows about it (S220). `None` where the kind has no sentence
/// yet, and the card falls back to the kind's own words.
pub fn question(kind: &str, name: &str, facts: &Facts) -> Option<String> {
    Some(match kind {
        "bolt-close" => match facts.units_merged {
            0 => format!("Nothing is left on {name}. Land it on main?"),
            1 => format!("Its one unit is merged. Land {name} on main?"),
            n => format!("All {n} units are merged. Land {name} on main?"),
        },
        "signal-unmoved" => "Build it, elaborate it as an intent, or drop it?".to_string(),
        "intent-proposed" => format!("Open {name} as an intent and start elaborating?"),
        "intent-close" => format!("Every elaboration of {name} has delivered. Close it?"),
        "question" => "The agent is waiting on your answer.".to_string(),
        "stalled" => format!("The session on {name} stalled."),
        "land-failed" => format!("Landing {name} on main failed."),
        "claim-moved" => format!("A claim {name} cites moved under it."),
        "unit-claim-moved" => format!("A claim {name} cites moved under it."),
        "package-install" => format!("Install {name} on this host?"),
        "app-coverage" | "app-install" | "package-secret" | "service-failed" => return None,
        _ => return None,
    })
}

/// What the page knows about the object a decision stands on, as far as the
/// question needs it.
#[derive(Default, Clone, Copy)]
pub struct Facts {
    pub units_merged: usize,
}

/// What the control says: the verb the answer performs, where the model's word
/// for it is not that verb (S220).
pub fn label(kind: &str, answer: &str) -> String {
    match (kind, answer) {
        ("bolt-close", "yes") => "land it".into(),
        ("bolt-close", "hold") => "hold".into(),
        ("intent-proposed", "yes") => "open".into(),
        ("package-install", "yes") => "install".into(),
        ("signal-unmoved", "build") => "build".into(),
        ("signal-unmoved", "intent") => "intent".into(),
        ("signal-unmoved", "drop") => "drop".into(),
        ("land-failed", "retry") | ("stalled", "retry") => "retry".into(),
        _ => super::said(answer),
    }
}

/// What pressing the control does, in one line, for the control's tooltip
/// (S220).
pub fn does(kind: &str, answer: &str) -> Option<&'static str> {
    Some(match (kind, answer) {
        ("bolt-close", "yes") => "merge the branch into main and remove its place",
        ("bolt-close", "hold") => "keep the bolt open; the decision returns when something merges",
        ("signal-unmoved", "build") => "a chore on a new bolt, worked by an agent in its own place",
        ("signal-unmoved", "intent") => "an intent to elaborate before anything is built",
        ("signal-unmoved", "drop") => "set it aside; it stays on record",
        ("intent-proposed", "yes") => "open the intent and start its elaborations",
        ("intent-proposed", "drop") => "set it aside with its signals",
        ("intent-proposed", "split") => "send the signals back to curation",
        ("intent-close", "close") => "close the intent; its elaborations stand",
        ("intent-close", "keep open") => "keep it open for more elaboration",
        ("stalled", "retry") => "start the session again in the same place",
        ("stalled", "hold") => "leave it as it stands",
        ("stalled", "drop") => "drop the work item",
        ("land-failed", "retry") => "try the landing again",
        ("land-failed", "hold") => "keep the bolt open",
        _ => return None,
    })
}

/// The word a board object's mark carries for a decision of this kind: what is
/// being asked, in one word, beside the number (S219).
pub fn short(kind: &str) -> &str {
    match kind {
        "signal-unmoved" => "what next",
        "bolt-close" => "land?",
        "intent-proposed" => "open?",
        "intent-close" => "close?",
        "question" => "question",
        "stalled" => "stalled",
        "land-failed" => "landing failed",
        "claim-moved" | "unit-claim-moved" => "claim moved",
        "package-install" => "install?",
        other => other,
    }
}

/// The affirmative answer of a decision, drawn as its primary control.
pub fn primary(answer: &str) -> bool {
    matches!(answer, "yes" | "build" | "installed" | "placed" | "close" | "start")
}

/// The answer that sets something aside, drawn in the colour for it.
pub fn dismisses(answer: &str) -> bool {
    matches!(answer, "drop" | "no" | "stop")
}

/// The key each answer is pressed with, one per answer and none twice on one
/// card (S56, S218). The first letter of the model's word where it is free,
/// the next letter of it otherwise; `j`, `k`, `o` and `/` belong to the rail
/// itself and are never an answer's.
pub fn keys(answers: &[String]) -> Vec<Option<char>> {
    const RESERVED: &[char] = &['j', 'k', 'o', '/'];
    let mut taken: Vec<char> = Vec::new();
    answers
        .iter()
        .map(|answer| {
            let word = super::said(answer).to_lowercase();
            // A question answered in a sentence has no key: typing is the
            // answer (S30).
            if super::is_all_argument(answer) {
                return None;
            }
            let picked = word
                .chars()
                .filter(|c| c.is_ascii_lowercase())
                .find(|c| !RESERVED.contains(c) && !taken.contains(c))?;
            taken.push(picked);
            Some(picked)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_first_free_letters_and_never_the_rails_own() {
        let answers: Vec<String> = ["yes", "hold", "keep open", "drop", "redo: <notes>", "<text>"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(
            keys(&answers),
            vec![Some('y'), Some('h'), Some('e'), Some('d'), Some('r'), None]
        );
    }

    #[test]
    fn a_bolt_close_reads_as_a_question_with_its_count() {
        let facts = Facts { units_merged: 1 };
        assert_eq!(
            question("bolt-close", "readme-fix", &facts).unwrap(),
            "Its one unit is merged. Land readme-fix on main?"
        );
        assert_eq!(label("bolt-close", "yes"), "land it");
        assert_eq!(label("stalled", "redo: <notes>"), "redo");
    }
}
