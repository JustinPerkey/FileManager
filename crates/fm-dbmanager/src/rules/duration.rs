//! Durations: the grammar (`[-]` then a bare integer, or components
//! `<digits><d|h|m|s|ms>` in strictly decreasing order, optionally separated by
//! single spaces), and parsing to and formatting from a count of a unit.

use crate::config::DurationUnit;

/// A duration that matched the grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parsed {
    /// A bare integer: a count of the field's unit.
    Count(i128),
    /// Components, as signed milliseconds.
    Millis(i128),
}

const SUFFIXES: [(&str, u128); 5] = [
    ("d", 86_400_000),
    ("h", 3_600_000),
    ("m", 60_000),
    ("s", 1_000),
    ("ms", 1),
];

/// Why [`grammar`] rejected a string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrammarError {
    /// Does not match the grammar; the text is a short reason.
    Syntax(String),
    /// Matches the grammar but is too large to hold.
    Overflow,
}

impl std::fmt::Display for GrammarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(why) => f.write_str(why),
            Self::Overflow => f.write_str("duration is too large"),
        }
    }
}

fn overflow() -> GrammarError {
    GrammarError::Overflow
}

fn syntax(why: impl Into<String>) -> GrammarError {
    GrammarError::Syntax(why.into())
}

/// Parses `s` against the grammar only.
///
/// # Errors
///
/// [`GrammarError`] when `s` does not match the grammar or is too large.
pub fn grammar(s: &str) -> Result<Parsed, GrammarError> {
    let (negative, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    if body.is_empty() {
        return Err(syntax("empty duration"));
    }
    let signed = |n: u128| -> Result<i128, GrammarError> {
        let n = i128::try_from(n).map_err(|_| overflow())?;
        Ok(if negative { -n } else { n })
    };
    if body.bytes().all(|b| b.is_ascii_digit()) {
        let n: u128 = body.parse().map_err(|_| overflow())?;
        return Ok(Parsed::Count(signed(n)?));
    }
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut last_rank: Option<usize> = None;
    let mut total: u128 = 0;
    loop {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            let found = body[start..]
                .chars()
                .next()
                .map_or(String::new(), String::from);
            return Err(syntax(format!("expected digits, found `{found}`")));
        }
        let digits: u128 = body[start..i].parse().map_err(|_| overflow())?;
        let suffix_start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let suffix = &body[suffix_start..i];
        let Some(rank) = SUFFIXES.iter().position(|(name, _)| *name == suffix) else {
            return Err(syntax(if suffix.is_empty() {
                "a number needs a unit suffix (d, h, m, s, ms)".to_string()
            } else {
                format!("unknown unit `{suffix}` (use d, h, m, s, ms)")
            }));
        };
        if last_rank.is_some_and(|last| rank <= last) {
            return Err(syntax(
                "units must go from largest to smallest, each at most once",
            ));
        }
        last_rank = Some(rank);
        total = digits
            .checked_mul(SUFFIXES[rank].1)
            .and_then(|v| total.checked_add(v))
            .ok_or_else(overflow)?;
        if i == bytes.len() {
            break;
        }
        if bytes[i] == b' ' {
            i += 1;
            if i == bytes.len() {
                return Err(syntax("trailing space"));
            }
        }
    }
    Ok(Parsed::Millis(signed(total)?))
}

/// Why [`parse`] rejected a duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationError {
    /// Does not match the grammar.
    Grammar,
    /// The count does not fit an `i64`.
    Overflow,
    /// Not a whole number of the unit.
    Precision,
}

impl DurationUnit {
    /// Milliseconds in one of this unit.
    #[must_use]
    pub const fn millis(self) -> i128 {
        match self {
            Self::Milliseconds => 1,
            Self::Seconds => 1_000,
            Self::Minutes => 60_000,
            Self::Hours => 3_600_000,
            Self::Days => 86_400_000,
        }
    }

    /// The suffix of this unit in the grammar.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Milliseconds => "ms",
            Self::Seconds => "s",
            Self::Minutes => "m",
            Self::Hours => "h",
            Self::Days => "d",
        }
    }

    /// The unit's name as written in the config (`"minutes"`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Milliseconds => "milliseconds",
            Self::Seconds => "seconds",
            Self::Minutes => "minutes",
            Self::Hours => "hours",
            Self::Days => "days",
        }
    }
}

/// Parses `text` to a count of `unit`.
///
/// # Errors
///
/// [`DurationError`] when the text breaks the grammar, is not a whole number
/// of `unit`, or does not fit an `i64`.
pub fn parse(text: &str, unit: DurationUnit) -> Result<i64, DurationError> {
    let count = match grammar(text) {
        Err(GrammarError::Syntax(_)) => return Err(DurationError::Grammar),
        Err(GrammarError::Overflow) => return Err(DurationError::Overflow),
        Ok(Parsed::Count(n)) => n,
        Ok(Parsed::Millis(ms)) => {
            if ms % unit.millis() != 0 {
                return Err(DurationError::Precision);
            }
            ms / unit.millis()
        }
    };
    i64::try_from(count).map_err(|_| DurationError::Overflow)
}

/// The canonical text of `count` of `unit`: the sign, then the non-zero
/// components from `d` down to the unit's own suffix, no spaces. Zero is `0`
/// plus the unit's suffix.
#[must_use]
pub fn format(count: i64, unit: DurationUnit) -> String {
    if count == 0 {
        return format!("0{}", unit.suffix());
    }
    let total = i128::from(count) * unit.millis();
    let mut rest = total.unsigned_abs();
    let mut out = String::new();
    if total < 0 {
        out.push('-');
    }
    let floor = unit.millis().unsigned_abs();
    for (suffix, size) in SUFFIXES {
        if size < floor {
            break;
        }
        let n = rest / size;
        rest %= size;
        if n > 0 {
            out.push_str(&n.to_string());
            out.push_str(suffix);
        }
    }
    out
}
