//! A deliverable read on the page: markdown as a document a person reads.
//!
//! A session that finishes leaves a file at a path, and the page links to it
//! (190, 213, D16). A markdown deliverable is rendered here, into the page, so
//! the operator reads what the session produced without leaving the object it
//! belongs to; an HTML deliverable is not rendered here at all, because it is
//! its own document with its own styles and is served at its own address.
//!
//! That split is a safety property as much as a design one. Everything this
//! renders is escaped before a single tag is written, so a deliverable cannot
//! put markup into the page that produced it — and the one kind of file that
//! *is* markup never enters the page, it is fetched on its own.
//!
//! The subset is what a session actually writes: headings, paragraphs, bullet
//! and numbered lists, fenced code, block quotes, tables and horizontal rules,
//! with `**bold**`, `*italic*`, `` `code` `` and `[text](link)` inside a line.
//! What is not recognised is left as the text it is, which is the right answer
//! for a document nobody promised would be markdown at all.

use crate::page::escape;
use std::fmt::Write as _;

/// Render a markdown document to the HTML the dock shows.
pub fn render(source: &str) -> String {
    let mut out = String::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut at = 0;
    // What block is open, so a run of bullets is one list and a run of
    // paragraph lines is one paragraph.
    let mut paragraph: Vec<&str> = Vec::new();
    let mut list: Option<&'static str> = None;

    // Close whatever is open before a new block starts. A paragraph that ran
    // into a heading and a list that ran into a paragraph are the two cases a
    // line-at-a-time renderer gets wrong.
    macro_rules! close {
        ($out:expr) => {{
            if !paragraph.is_empty() {
                let _ = write!($out, "<p>{}</p>\n", inline(&paragraph.join(" ")));
                paragraph.clear();
            }
            if let Some(tag) = list.take() {
                let _ = write!($out, "</{tag}>\n");
            }
        }};
    }

    while at < lines.len() {
        let line = lines[at];
        let trimmed = line.trim_start();

        // A fence runs to its closing fence, and nothing inside it is markdown.
        if let Some(language) = trimmed.strip_prefix("```") {
            close!(out);
            at += 1;
            let mut held = Vec::new();
            while at < lines.len() && !lines[at].trim_start().starts_with("```") {
                held.push(lines[at]);
                at += 1;
            }
            at += 1;
            let _ = write!(
                out,
                "<pre class=\"code\" data-language=\"{}\"><code>{}</code></pre>\n",
                escape(language.trim()),
                escape(&held.join("\n"))
            );
            continue;
        }

        // A table is a header row, a rule, and the rows under it. It is read
        // whole because the rule alone says the row above it was a header.
        if trimmed.starts_with('|') && at + 1 < lines.len() && is_table_rule(lines[at + 1]) {
            close!(out);
            out.push_str("<table><thead><tr>");
            for cell in cells(trimmed) {
                let _ = write!(out, "<th>{}</th>", inline(&cell));
            }
            out.push_str("</tr></thead><tbody>\n");
            at += 2;
            while at < lines.len() && lines[at].trim_start().starts_with('|') {
                out.push_str("<tr>");
                for cell in cells(lines[at].trim_start()) {
                    let _ = write!(out, "<td>{}</td>", inline(&cell));
                }
                out.push_str("</tr>\n");
                at += 1;
            }
            out.push_str("</tbody></table>\n");
            continue;
        }

        if trimmed.is_empty() {
            close!(out);
            at += 1;
            continue;
        }

        if trimmed.starts_with("---") || trimmed.starts_with("***") {
            if trimmed.chars().all(|c| c == '-' || c == '*' || c == ' ') {
                close!(out);
                out.push_str("<hr>\n");
                at += 1;
                continue;
            }
        }

        if let Some(level) = heading(trimmed) {
            close!(out);
            let text = trimmed[level..].trim();
            // Headings past the sixth are not headings; the document said what
            // it said and it is a paragraph.
            let level = level.min(6);
            let _ = write!(out, "<h{level}>{}</h{level}>\n", inline(text));
            at += 1;
            continue;
        }

        if let Some(said) = trimmed.strip_prefix("> ").or_else(|| trimmed.strip_prefix(">")) {
            close!(out);
            let _ = write!(out, "<blockquote>{}</blockquote>\n", inline(said.trim()));
            at += 1;
            continue;
        }

        if let Some(item) = bullet(trimmed) {
            if !paragraph.is_empty() {
                let _ = write!(out, "<p>{}</p>\n", inline(&paragraph.join(" ")));
                paragraph.clear();
            }
            if list != Some("ul") {
                if let Some(tag) = list.take() {
                    let _ = write!(out, "</{tag}>\n");
                }
                out.push_str("<ul>\n");
                list = Some("ul");
            }
            at += 1;
            let _ = write!(out, "<li>{}</li>\n", inline(&continued(&lines, &mut at, item)));
            continue;
        }

        if let Some(item) = numbered(trimmed) {
            if !paragraph.is_empty() {
                let _ = write!(out, "<p>{}</p>\n", inline(&paragraph.join(" ")));
                paragraph.clear();
            }
            if list != Some("ol") {
                if let Some(tag) = list.take() {
                    let _ = write!(out, "</{tag}>\n");
                }
                out.push_str("<ol>\n");
                list = Some("ol");
            }
            at += 1;
            let _ = write!(out, "<li>{}</li>\n", inline(&continued(&lines, &mut at, item)));
            continue;
        }

        // Anything else is a line of a paragraph. A list the paragraph
        // interrupts is closed first.
        if let Some(tag) = list.take() {
            let _ = write!(out, "</{tag}>\n");
        }
        paragraph.push(trimmed);
        at += 1;
    }
    close!(out);
    out
}

/// An item's own wrapped lines, which belong to it rather than to a paragraph
/// after it: a bullet in a hand-written document is very often three lines.
fn continued<'a>(lines: &[&'a str], at: &mut usize, first: &'a str) -> String {
    let mut held = vec![first.trim().to_string()];
    while *at < lines.len() {
        let line = lines[*at];
        let trimmed = line.trim_start();
        let indented = line.len() - trimmed.len() >= 2;
        if trimmed.is_empty()
            || !indented
            || bullet(trimmed).is_some()
            || numbered(trimmed).is_some()
            || heading(trimmed).is_some()
        {
            break;
        }
        held.push(trimmed.to_string());
        *at += 1;
    }
    held.join(" ")
}

/// How many `#` a heading opens with, or nothing where it is not one.
fn heading(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    match hashes > 0 && line[hashes..].starts_with(' ') {
        true => Some(hashes),
        false => None,
    }
}

/// What a bullet says, without its mark.
fn bullet(line: &str) -> Option<&str> {
    for mark in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(mark) {
            return Some(rest);
        }
    }
    None
}

/// What a numbered item says, without its number.
fn numbered(line: &str) -> Option<&str> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    line[digits..].strip_prefix(". ").or_else(|| line[digits..].strip_prefix(") "))
}

/// Whether a line is the rule under a table's header row.
fn is_table_rule(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('|')
        && line.contains('-')
        && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// One table row's cells, without the pipes that bound them.
fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// What a line says, with its inline marks read.
///
/// The text is escaped **first** and the tags are written after, so a
/// deliverable that contains `<script>` reads as those characters and is never
/// markup in the page it is shown on.
fn inline(said: &str) -> String {
    let escaped = escape(said);
    // Code first: what is inside a span of code is the characters it holds and
    // no mark inside it is read.
    let mut out = String::new();
    let mut rest = escaped.as_str();
    while let Some(open) = rest.find('`') {
        out.push_str(&marks(&rest[..open]));
        let after = &rest[open + 1..];
        match after.find('`') {
            Some(close) => {
                let _ = write!(out, "<code>{}</code>", &after[..close]);
                rest = &after[close + 1..];
            }
            // An unmatched backtick is a backtick.
            None => {
                out.push('`');
                rest = after;
            }
        }
    }
    out.push_str(&marks(rest));
    out
}

/// The marks that are read outside a span of code: strong, emphasis and links.
fn marks(said: &str) -> String {
    let said = pairs(said, "**", "strong");
    let said = pairs(&said, "__", "strong");
    let said = pairs(&said, "*", "em");
    links(&said)
}

/// Text between two of the same mark becomes an element; an unmatched mark is
/// left as the character it is.
fn pairs(said: &str, mark: &str, tag: &str) -> String {
    let mut out = String::new();
    let mut rest = said;
    loop {
        let Some(open) = rest.find(mark) else {
            out.push_str(rest);
            return out;
        };
        let after = &rest[open + mark.len()..];
        let Some(close) = after.find(mark) else {
            out.push_str(&rest[..open + mark.len()]);
            rest = after;
            continue;
        };
        // `**` inside a search for `*` would make an empty element; an empty
        // one says nothing and the marks stay as they are.
        if close == 0 {
            out.push_str(&rest[..open + mark.len()]);
            rest = after;
            continue;
        }
        out.push_str(&rest[..open]);
        let _ = write!(out, "<{tag}>{}</{tag}>", &after[..close]);
        rest = &after[close + mark.len()..];
    }
}

/// `[text](target)`. A target that is not plainly a path or an http address is
/// left as the text it is: a document is read on the page, and a link out of it
/// is not a place to accept whatever a file says (310).
fn links(said: &str) -> String {
    let mut out = String::new();
    let mut rest = said;
    while let Some(open) = rest.find('[') {
        let after = &rest[open..];
        let read = after
            .find("](")
            .and_then(|shut| after[shut + 2..].find(')').map(|end| (shut, shut + 2 + end)));
        let Some((shut, end)) = read else {
            out.push_str(&rest[..open + 1]);
            rest = &rest[open + 1..];
            continue;
        };
        let text = &after[1..shut];
        let target = &after[shut + 2..end];
        out.push_str(&rest[..open]);
        match safe_target(target) {
            true => {
                let _ = write!(out, "<a href=\"{target}\" rel=\"noopener\">{text}</a>");
            }
            false => out.push_str(text),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Whether a link target is one the page will write. A relative path or an
/// http address; never a scheme a document chose for itself.
fn safe_target(target: &str) -> bool {
    if target.is_empty() || target.contains(['"', '\'', ' ', '<', '>']) {
        return false;
    }
    target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with('#')
        || (!target.contains(':') && !target.starts_with("//"))
}

#[cfg(test)]
mod tests {
    use super::render;

    /// The blocks a session actually writes, each one its own element (190).
    #[test]
    fn a_document_renders_as_the_blocks_it_is_written_in() {
        let out = render(
            "# Why cards decline\n\
             \n\
             The rate rose from 1.1 per cent to 4.2 per cent over eight days.\n\
             It is one shape.\n\
             \n\
             ## What is declining\n\
             \n\
             - **American Express** is 9 per cent of attempts.\n\
             - The declines cluster in a band of amounts.\n\
             \n\
             1. Replay the fourteen days.\n\
             2. Measure what a schedule would have recovered.\n",
        );
        assert!(out.contains("<h1>Why cards decline</h1>"), "{out}");
        assert!(out.contains("<h2>What is declining</h2>"), "{out}");
        // Two wrapped lines are one paragraph, not two.
        assert!(
            out.contains("<p>The rate rose from 1.1 per cent to 4.2 per cent over eight days. It is one shape.</p>"),
            "{out}"
        );
        assert_eq!(out.matches("<ul>").count(), 1, "the bullets are one list: {out}");
        assert_eq!(out.matches("<ol>").count(), 1, "the numbered items are one list: {out}");
        assert_eq!(out.matches("<li>").count(), 4, "{out}");
        assert!(out.contains("<strong>American Express</strong>"), "{out}");
    }

    /// A bullet that wraps belongs to its bullet and not to a paragraph after
    /// it, which is how a hand-written document is actually laid out.
    #[test]
    fn a_wrapped_bullet_stays_one_item() {
        let out = render(
            "- **Amex is 9 per cent of attempts and 38 per cent of declines.**\n  \
             Before the eighth of August it was 11 per cent of declines.\n\
             - The second attempt declines with the first.\n",
        );
        assert_eq!(out.matches("<li>").count(), 2, "{out}");
        assert!(out.contains("Before the eighth of August"), "{out}");
        assert!(!out.contains("<p>Before"), "the continuation became a paragraph: {out}");
    }

    /// Code, emphasis and a table, and a mark with no partner left as itself.
    #[test]
    fn the_inline_marks_and_a_table_are_read() {
        let out = render(
            "The boolean the service writes as `declined`, and *this* is *emphasis*.\n\
             \n\
             | brand | declines |\n\
             | --- | --- |\n\
             | amex | 38% |\n",
        );
        assert!(out.contains("<code>declined</code>"), "{out}");
        assert!(out.contains("<em>this</em>") && out.contains("<em>emphasis</em>"), "{out}");
        assert!(out.contains("<thead><tr><th>brand</th><th>declines</th></tr></thead>"), "{out}");
        assert!(out.contains("<td>amex</td>"), "{out}");

        let lonely = render("2 * 3 is 6 and a `backtick with no partner\n");
        assert!(!lonely.contains("<em>"), "an unmatched mark became an element: {lonely}");
        assert!(lonely.contains('`'), "an unmatched backtick is a backtick: {lonely}");
    }

    /// Nothing a deliverable holds becomes markup in the page that shows it.
    ///
    /// The page renders a file somebody's session wrote. It is escaped before a
    /// tag is written, so the one kind of file that *is* markup — an HTML
    /// deliverable — is the kind the page never renders and always serves at
    /// its own address (310).
    #[test]
    fn a_deliverable_cannot_write_the_page_that_shows_it() {
        let out = render(
            "# <script>alert(1)</script>\n\
             \n\
             An <img src=x onerror=alert(1)> in a paragraph, and `<b>` in code.\n\
             \n\
             - <iframe src=\"evil\"></iframe>\n",
        );
        // Every `<` left in the output opens a tag this renderer wrote, and the
        // vocabulary it writes is small and fixed. That is the property worth
        // asserting: a substring check would pass over `onerror=` sitting
        // harmlessly inside escaped text and fail over nothing at all.
        let ours = [
            "p", "/p", "h1", "/h1", "h2", "/h2", "h3", "/h3", "h4", "/h4", "h5", "/h5", "h6",
            "/h6", "ul", "/ul", "ol", "/ol", "li", "/li", "strong", "/strong", "em", "/em",
            "code", "/code", "pre", "/pre", "blockquote", "/blockquote", "hr", "table", "/table",
            "thead", "/thead", "tbody", "/tbody", "tr", "/tr", "th", "/th", "td", "/td", "a", "/a",
        ];
        for opened in out.split('<').skip(1) {
            let name = opened
                .split([' ', '>'])
                .next()
                .expect("a tag opens with a name");
            assert!(
                ours.contains(&name),
                "`<{name}` is in the page and this renderer does not write it: {out}"
            );
        }
        // And what the file said is shown as the text it is, not dropped.
        assert!(out.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "{out}");
        assert!(out.contains("&lt;img src=x onerror=alert(1)&gt;"), "{out}");
        assert!(out.contains("<code>&lt;b&gt;</code>"), "{out}");
    }

    /// A link is written only where its target is one the page would write; a
    /// document does not get to choose a scheme (310).
    #[test]
    fn a_link_is_written_only_to_a_target_the_page_would_write() {
        let out = render(
            "[the note](research/declines.md) and [the site](https://example.com/x) \
             and [no](javascript:alert(1)) and [nor](data:text/html,x)\n",
        );
        assert!(out.contains("<a href=\"research/declines.md\""), "{out}");
        assert!(out.contains("<a href=\"https://example.com/x\""), "{out}");
        assert!(!out.contains("javascript:"), "{out}");
        assert!(!out.contains("data:text/html"), "{out}");
        // And the text of a refused link is still read.
        assert!(out.contains("no") && out.contains("nor"), "{out}");
    }

    /// Nothing in a fence is markdown, and the fence keeps its language.
    #[test]
    fn a_fence_holds_what_it_holds() {
        let out = render("```rust\nlet x = *p;\n# not a heading\n```\n");
        assert!(out.contains("data-language=\"rust\""), "{out}");
        assert!(out.contains("# not a heading"), "{out}");
        assert!(!out.contains("<h1>"), "{out}");
        assert!(!out.contains("<em>"), "{out}");
    }
}
