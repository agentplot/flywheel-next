//! The palette's commands: what a leading `/` in the one typed input names,
//! in the operator's words (19, 193, S30, S31, S63).
//!
//! A command is a tool of the catalogue by its own name, and nothing else: the
//! slash chooses the tool, and what is typed after the name is that tool's
//! argument, sent as typed. A tool that acts on an object takes the one in
//! hand — the decision the rail holds, or the object the dock has open — or an
//! id typed after the name. The page reads no word of plain text for meaning:
//! plain text is a capture, and a bare number is the chat's reply grammar,
//! answering the decision it names with the controls its card carries.
//!
//! What is left out is what the palette cannot seed as one call: `propose-unit`
//! and `open-session` are the capture's and the repository's own controls with
//! their fields, `start` and `stop` wait on a service the page shows, and
//! removing the instance is not one Enter away. `/curate` takes nothing and is
//! the call the signals tray's control makes (110).

use super::escape;
use std::fmt::Write as _;

/// What a command takes from what is typed after it, or from what is in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Takes {
    /// The object in hand, or an id typed after the name, as this argument.
    Object(&'static str),
    /// The number of the decision in hand, or a number typed after the name.
    Decision(&'static str),
    /// A number and then the answer, as the chat's reply grammar has them:
    /// `/answer 412 yes`.
    Answer,
    /// The words typed after the name, whole, as this argument.
    Words(&'static str),
    /// The object in hand as the first argument, and the words typed after the
    /// name as the second.
    ObjectAndWords(&'static str, &'static str),
    /// The first word typed after the name as the first argument, and the rest
    /// as the second: `/ask atlas the rows lose their numbers`.
    NameAndWords(&'static str, &'static str),
    /// Nothing: the command is the whole call, `/curate`.
    Nothing,
}

impl Takes {
    /// The argument names this sends, in the catalogue's order.
    pub fn args(&self) -> Vec<&'static str> {
        match *self {
            Takes::Nothing => vec![],
            Takes::Object(a) | Takes::Decision(a) | Takes::Words(a) => vec![a],
            Takes::Answer => vec!["decision", "answer"],
            Takes::ObjectAndWords(a, b) | Takes::NameAndWords(a, b) => vec![a, b],
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Takes::Object(_) => "object",
            Takes::Decision(_) => "decision",
            Takes::Answer => "answer",
            Takes::Words(_) => "words",
            Takes::ObjectAndWords(..) => "object-words",
            Takes::NameAndWords(..) => "name-words",
            Takes::Nothing => "none",
        }
    }
}

/// One command: the tool it calls, what is typed after it, what it does and
/// what it acts on.
#[derive(Debug, Clone, Copy)]
pub struct Command {
    pub tool: &'static str,
    pub typed: &'static str,
    pub does: &'static str,
    pub on: &'static str,
    pub takes: Takes,
}

/// The commands, in the order the list shows them before anything is typed.
pub const COMMANDS: &[Command] = &[
    Command {
        tool: "answer",
        typed: "<number> <answer>",
        does: "answer a decision by its number",
        on: "the decision you name",
        takes: Takes::Answer,
    },
    Command {
        tool: "capture",
        typed: "<words>",
        does: "capture what you noticed, exactly as you wrote it",
        on: "this flywheel",
        takes: Takes::Words("text"),
    },
    Command {
        tool: "ask",
        typed: "<repository> <words>",
        does: "ask for work in a repository",
        on: "a repository",
        takes: Takes::NameAndWords("repository", "text"),
    },
    Command {
        tool: "curate",
        typed: "",
        does: "run curation now over the notes waiting",
        on: "this flywheel",
        takes: Takes::Nothing,
    },
    Command {
        tool: "drop",
        typed: "",
        does: "say no; it stops here and stays on record",
        on: "what is in hand",
        takes: Takes::Object("object"),
    },
    Command {
        tool: "later",
        typed: "",
        does: "set the decision aside and ask again in a week",
        on: "the decision in hand",
        takes: Takes::Decision("decision"),
    },
    Command {
        tool: "hold",
        typed: "",
        does: "keep it where it is until you release it",
        on: "what is in hand",
        takes: Takes::Object("object"),
    },
    Command {
        tool: "release",
        typed: "",
        does: "let go of a hold",
        on: "what is in hand",
        takes: Takes::Object("object"),
    },
    Command {
        tool: "rename",
        typed: "<name>",
        does: "give the bolt another name",
        on: "the bolt in hand",
        takes: Takes::ObjectAndWords("bolt", "name"),
    },
    Command {
        tool: "close",
        typed: "",
        does: "close it, where its close is offered",
        on: "the bolt or intent in hand",
        takes: Takes::Object("object"),
    },
    Command {
        tool: "finish",
        typed: "",
        does: "finish a standing elaboration without waiting",
        on: "the elaboration in hand",
        takes: Takes::Object("elaboration"),
    },
    Command {
        tool: "end",
        typed: "",
        does: "end the session you are working with",
        on: "the session in hand",
        takes: Takes::Object("session"),
    },
    Command {
        tool: "retire",
        typed: "",
        does: "stop it for good",
        on: "what is in hand",
        takes: Takes::Object("object"),
    },
    Command {
        tool: "revive",
        typed: "",
        does: "bring a set-aside note back for curation",
        on: "the note in hand",
        takes: Takes::Object("signal"),
    },
    Command {
        tool: "take",
        typed: "<branch>",
        does: "bring main into the branch now",
        on: "the branch you name",
        takes: Takes::Object("line"),
    },
    Command {
        tool: "takeover",
        typed: "<host>",
        does: "move a lost host's work onto one that is running",
        on: "the host you name",
        takes: Takes::Object("host"),
    },
];

/// The commands as the page carries them: a template the page's script lists
/// from when `/` is typed. Nothing in it is state; it is the catalogue as this
/// page may invoke it (193, 310).
pub fn template() -> String {
    let mut out = String::from("<template class=\"pal-commands\">\n");
    for command in COMMANDS {
        let args = command.takes.args();
        let _ = write!(
            out,
            "<button type=\"button\" class=\"pal-row\" data-tool=\"{tool}\" data-takes=\"{takes}\" \
             data-arg=\"{arg}\" data-arg2=\"{arg2}\"><span class=\"c\">/{tool}</span>\
             <span class=\"arg\">{typed}</span><span class=\"w\">{does}</span>\
             <span class=\"o\">{on}</span></button>\n",
            tool = escape(command.tool),
            takes = command.takes.kind(),
            arg = args.first().copied().unwrap_or_default(),
            arg2 = args.get(1).copied().unwrap_or_default(),
            typed = escape(command.typed),
            does = escape(command.does),
            on = escape(command.on),
        );
    }
    out.push_str("</template>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Every command is a tool of the catalogue by its own name, sending only
    /// arguments that tool takes, and none is offered twice (193, S63).
    #[test]
    fn every_command_is_a_catalogue_tool_with_its_own_arguments() {
        let mut seen = BTreeSet::new();
        for command in COMMANDS {
            assert!(seen.insert(command.tool), "/{} is offered twice", command.tool);
            let tool = crate::catalogue::catalogue()
                .iter()
                .find(|t| t.name == command.tool)
                .unwrap_or_else(|| panic!("/{} names no tool of the catalogue", command.tool));
            for arg in command.takes.args() {
                assert!(
                    tool.args.contains(&arg),
                    "/{} sends `{arg}`, which the tool does not take: {:?}",
                    command.tool,
                    tool.args
                );
            }
        }
        let listed = template();
        for command in COMMANDS {
            assert!(listed.contains(&format!("data-tool=\"{}\"", command.tool)));
            assert!(listed.contains(&format!("<span class=\"c\">/{}</span>", command.tool)));
        }
    }
}
