//! A cadence: when a thing charged on a schedule is next due (110, 231).
//!
//! Every timed behaviour is a guard on the tick and nothing else keeps time
//! (231), so a cadence is read the same way any other evidence is: what the
//! schedule's last firing was at or before now, against when the thing last
//! ran. A run missed while the host was down is therefore caught up on the next
//! tick — once, however many firings were missed, because one run moves the
//! mark past all of them (231, 111).
//!
//! The schedule is written as cron's five fields, which is what
//! `flywheel.yaml curation.cadence` carries (`blueprints.yaml` evidence).

use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};

/// The cadence curation runs on where the record names none
/// (`blueprints.yaml` evidence.curation.cadence_due).
pub const DEFAULT: &str = "0 6 * * 1-5";

/// One schedule: minute, hour, day of month, month, day of week.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cadence {
    minute: Field,
    hour: Field,
    day: Field,
    month: Field,
    weekday: Field,
}

/// One field of the schedule: every value, or the ones it names.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Field {
    Every,
    These(Vec<u32>),
}

impl Field {
    fn parse(text: &str, low: u32, high: u32) -> Option<Field> {
        if text == "*" {
            return Some(Field::Every);
        }
        let mut values = Vec::new();
        for part in text.split(',') {
            match part.split_once('-') {
                Some((from, to)) => {
                    let (from, to) = (from.parse::<u32>().ok()?, to.parse::<u32>().ok()?);
                    if from > to || from < low || to > high {
                        return None;
                    }
                    values.extend(from..=to);
                }
                None => {
                    let one = part.parse::<u32>().ok()?;
                    if one < low || one > high {
                        return None;
                    }
                    values.push(one);
                }
            }
        }
        match values.is_empty() {
            true => None,
            false => Some(Field::These(values)),
        }
    }

    fn holds(&self, value: u32) -> bool {
        match self {
            Field::Every => true,
            Field::These(these) => these.contains(&value),
        }
    }
}

impl Cadence {
    /// Read a schedule, or nothing where the text is not one. A cadence the
    /// machinery cannot read is no cadence: it fires never, rather than firing
    /// at a time nobody asked for.
    pub fn parse(text: &str) -> Option<Cadence> {
        let fields: Vec<&str> = text.split_whitespace().collect();
        let [minute, hour, day, month, weekday] = fields.as_slice() else {
            return None;
        };
        Some(Cadence {
            minute: Field::parse(minute, 0, 59)?,
            hour: Field::parse(hour, 0, 23)?,
            day: Field::parse(day, 1, 31)?,
            month: Field::parse(month, 1, 12)?,
            // Sunday is 0, as cron writes it.
            weekday: Field::parse(weekday, 0, 7)?,
        })
    }

    /// Whether the schedule fires at this minute.
    pub fn fires_at(&self, at: DateTime<Utc>) -> bool {
        let weekday = at.weekday().num_days_from_sunday();
        self.minute.holds(at.minute())
            && self.hour.holds(at.hour())
            && self.day.holds(at.day())
            && self.month.holds(at.month())
            && (self.weekday.holds(weekday) || (weekday == 0 && self.weekday.holds(7)))
    }

    /// The last time it fired at or before `now`, looked for over the year
    /// behind it. Nothing further back matters: a cadence that has not fired in
    /// a year is one nothing is waiting on.
    pub fn last_firing(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let mut at = Utc
            .with_ymd_and_hms(now.year(), now.month(), now.day(), now.hour(), now.minute(), 0)
            .single()?;
        for _ in 0..(366 * 24 * 60) {
            if self.fires_at(at) {
                return Some(at);
            }
            at -= Duration::minutes(1);
        }
        None
    }
}

/// Whether a cadence has fired since something last ran (110, 231).
///
/// One run catches up however many firings were missed while the host was
/// down, because the run moves `since` past all of them: the catch-up writes
/// nothing twice (231, 111).
pub fn due(cadence: &str, since: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let Some(cadence) = Cadence::parse(cadence) else {
        return false;
    };
    cadence
        .last_firing(now)
        .is_some_and(|fired| fired > since)
}
