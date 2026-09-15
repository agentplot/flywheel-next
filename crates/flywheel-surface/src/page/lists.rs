//! Every list on the page shows its first fifty rows with its count and a
//! `more` that fetches the next fifty from the host; the rail is never paged,
//! since every decision is in its count (310a, S235, 15, S9).

use super::escape;

/// How many rows a list shows, and how many a `more` fetches.
pub const PAGE: usize = 50;

/// What one row of a list is, so the `more` stands where a row would.
#[derive(Debug, Clone, Copy)]
pub enum Row {
    Div,
    Li,
    /// A table row, spanning the table's columns.
    Tr(usize),
    Span,
}

/// Rows `from` to `from + PAGE` of a list, and its `more` where rows remain.
/// `object` is the object whose list it is, or none for the page's own.
pub fn page(rows: &[String], from: usize, object: Option<&str>, list: &str, row: Row) -> String {
    page_with(rows, from, object, list, row, |drawn| drawn.clone())
}

/// The same over rows drawn as they are shown: a list of thousands draws the
/// fifty it shows and none of the rest, so what a page costs grows with what is
/// on screen (310a, S235).
pub fn page_with<T>(
    items: &[T],
    from: usize,
    object: Option<&str>,
    list: &str,
    row: Row,
    draw: impl Fn(&T) -> String,
) -> String {
    let end = (from + PAGE).min(items.len());
    let mut out: String = items.get(from.min(end)..end).map(|shown| shown.iter().map(&draw).collect()).unwrap_or_default();
    if end < items.len() {
        out.push_str(&more(end, items.len(), object, list, row));
    }
    out
}

/// The control under a list's shown rows: how many of how many are shown, and
/// `more`, which fetches the next fifty (310a, S235).
pub fn more(shown: usize, count: usize, object: Option<&str>, list: &str, row: Row) -> String {
    let attributes = format!(
        " class=\"more\" data-list=\"{}\" data-object=\"{}\" data-from=\"{shown}\"",
        escape(list),
        escape(object.unwrap_or_default())
    );
    let inner = format!(
        "<span class=\"shown\">{shown} of {count}</span><button type=\"button\" class=\"btn sm quiet\">more</button>"
    );
    match row {
        Row::Div => format!("<div{attributes}>{inner}</div>\n"),
        Row::Li => format!("<li{attributes}>{inner}</li>\n"),
        Row::Tr(columns) => format!("<tr{attributes}><td colspan=\"{columns}\">{inner}</td></tr>\n"),
        Row::Span => format!("<span{attributes}>{inner}</span>\n"),
    }
}
