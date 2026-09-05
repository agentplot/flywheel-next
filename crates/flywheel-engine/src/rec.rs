//! A reader and writer for the subset of the recutils format the flywheel writes:
//! `%rec: kind`, `field: value`, `+ continuation`, blank line between records,
//! `#` comments. Nothing links the GNU tools.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Record {
    pub kind: Option<String>,
    /// Field order preserved; a repeated field appends.
    pub fields: Vec<(String, String)>,
}

impl Record {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    pub fn all(&self, name: &str) -> Vec<&str> {
        self.fields.iter().filter(|(k, _)| k == name).map(|(_, v)| v.as_str()).collect()
    }
    pub fn set(&mut self, name: &str, value: &str) {
        if let Some(f) = self.fields.iter_mut().find(|(k, _)| k == name) {
            f.1 = value.to_string();
        } else {
            self.fields.push((name.to_string(), value.to_string()));
        }
    }
    pub fn as_map(&self) -> BTreeMap<String, String> {
        self.fields.iter().cloned().collect()
    }
}

pub fn parse(text: &str) -> Vec<Record> {
    let mut out = Vec::new();
    let mut cur: Option<Record> = None;
    let mut kind: Option<String> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            if let Some(r) = cur.take() { out.push(r); }
            continue;
        }
        if line.starts_with('#') { continue; }
        if let Some(k) = line.strip_prefix("%rec:") {
            if let Some(r) = cur.take() { out.push(r); }
            kind = Some(k.trim().to_string());
            continue;
        }
        if line.starts_with('%') { continue; }
        if let Some(cont) = line.strip_prefix('+') {
            if let Some(r) = cur.as_mut() {
                if let Some(last) = r.fields.last_mut() {
                    if !last.1.is_empty() { last.1.push('\n'); }
                    last.1.push_str(cont.strip_prefix(' ').unwrap_or(cont));
                }
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let r = cur.get_or_insert_with(|| Record { kind: kind.clone(), fields: vec![] });
            r.fields.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    if let Some(r) = cur.take() { out.push(r); }
    out
}

pub fn write(records: &[Record]) -> String {
    let mut s = String::new();
    let mut last_kind: Option<&str> = None;
    for r in records {
        if r.kind.as_deref() != last_kind {
            if let Some(k) = &r.kind { s.push_str(&format!("%rec: {k}\n\n")); }
            last_kind = r.kind.as_deref();
        }
        for (k, v) in &r.fields {
            let mut lines = v.split('\n');
            s.push_str(&format!("{k}: {}\n", lines.next().unwrap_or("")));
            for l in lines { s.push_str(&format!("+ {l}\n")); }
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let text = "%rec: object\n\nid: unit/atlas/x\nstate: life=proposed\nnote: one\n+ two\n\nid: b\n";
        let recs = parse(text);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].get("note"), Some("one\ntwo"));
        assert_eq!(recs[0].kind.as_deref(), Some("object"));
        let again = parse(&write(&recs));
        assert_eq!(recs, again);
    }
}
