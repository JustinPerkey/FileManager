//! Pieces of resolving a field's rule: the pattern dialect check and
//! canonical bound text.

/// What the pattern scanner found.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PatternScan {
    /// The first shorthand class, written as in the pattern (`\d`).
    pub shorthand: Option<String>,
    /// An unescaped `.` outside a character class.
    pub dot: bool,
}

/// Looks for the constructs Rust's `regex` and JavaScript's `RegExp` read
/// differently. Tracks backslash escapes and bracket depth; it does not parse
/// the pattern.
pub(super) fn scan_pattern(pattern: &str) -> PatternScan {
    let mut scan = PatternScan::default();
    let mut depth = 0_u32;
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    if "dDwWsSbB".contains(next) && scan.shorthand.is_none() {
                        scan.shorthand = Some(format!("\\{next}"));
                    }
                }
            }
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            '.' if depth == 0 => scan.dot = true,
            _ => {}
        }
    }
    scan
}

/// Finite decimal text for a real bound, matching the real grammar.
pub(super) fn real_text(x: f64) -> String {
    format!("{x}")
}
