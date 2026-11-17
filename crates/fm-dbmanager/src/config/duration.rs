//! The duration grammar: `[-]` then a bare integer, or components
//! `<digits><d|h|m|s|ms>` in strictly decreasing order, optionally separated by
//! single spaces.

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

fn overflow() -> String {
    "duration is too large".into()
}

/// Parses `s`.
///
/// # Errors
///
/// A short reason when `s` does not match the grammar.
pub fn parse(s: &str) -> Result<Parsed, String> {
    let (negative, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    if body.is_empty() {
        return Err("empty duration".into());
    }
    let signed = |n: u128| -> Result<i128, String> {
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
            return Err(format!("expected digits, found `{found}`"));
        }
        let digits: u128 = body[start..i].parse().map_err(|_| overflow())?;
        let suffix_start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let suffix = &body[suffix_start..i];
        let Some(rank) = SUFFIXES.iter().position(|(name, _)| *name == suffix) else {
            return Err(if suffix.is_empty() {
                "a number needs a unit suffix (d, h, m, s, ms)".into()
            } else {
                format!("unknown unit `{suffix}` (use d, h, m, s, ms)")
            });
        };
        if last_rank.is_some_and(|last| rank <= last) {
            return Err("units must go from largest to smallest, each at most once".into());
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
                return Err("trailing space".into());
            }
        }
    }
    Ok(Parsed::Millis(signed(total)?))
}
