//! Pieces of resolving a field's rule: the pattern dialect check and
//! canonical bound text.

/// What the pattern scanner found.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PatternScan {
    /// The first construct Rust's `regex` and JavaScript's `RegExp` read
    /// differently, or one that only one of them accepts, as a clause that
    /// follows "`pattern` ". `None` when the pattern is in the common subset.
    pub problem: Option<String>,
    /// An unescaped `.` outside a character class.
    pub dot: bool,
}

const SHORTHAND_HINT: &str = "write an explicit class: `[0-9]`, `[A-Za-z0-9_]`, `[ \\t\\r\\n]`";

fn differs(what: &str) -> String {
    format!(
        "uses {what}, which Rust and JavaScript do not read the same way; use only the common subset the reference lists"
    )
}

/// Checks one escape, `\` already consumed and `rest` starting after it.
/// Returns how many characters the escape takes after the backslash, or the
/// problem.
fn escape(rest: &[char]) -> Result<usize, String> {
    let Some(&c) = rest.first() else {
        // A trailing backslash does not compile; the compile check says so.
        return Ok(0);
    };
    match c {
        'd' | 'D' | 'w' | 'W' | 's' | 'S' | 'b' | 'B' => Err(format!(
            "uses the shorthand class `\\{c}`, which Rust and JavaScript read differently; {SHORTHAND_HINT}"
        )),
        'A' | 'z' | 'Z' => Err(differs(&format!("the anchor `\\{c}`"))),
        'p' | 'P' => Err(differs(&format!("the property class `\\{c}`"))),
        '1'..='9' => Err(differs(&format!("the backreference `\\{c}`"))),
        '\\' | '^' | '$' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
        | '/' | '-' | 't' | 'n' | 'r' | 'f' | 'v' => Ok(1),
        'x' => {
            if rest.len() >= 3 && rest[1].is_ascii_hexdigit() && rest[2].is_ascii_hexdigit() {
                Ok(3)
            } else {
                Err(differs("`\\x` without two hex digits"))
            }
        }
        other => Err(differs(&format!("the escape `\\{other}`"))),
    }
}

fn note(scan: &mut PatternScan, p: String) {
    if scan.problem.is_none() {
        scan.problem = Some(p);
    }
}

/// Looks for the constructs Rust's `regex` and JavaScript's `RegExp` read
/// differently or accept differently. Tracks backslash escapes and bracket
/// depth; it does not parse the pattern.
pub(super) fn scan_pattern(pattern: &str) -> PatternScan {
    let chars: Vec<char> = pattern.chars().collect();
    let mut scan = PatternScan::default();
    let mut in_class = false;
    // The next character is the first one of a class, where `]` is a literal.
    let mut class_start = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let was_start = class_start;
        class_start = false;
        i += 1;
        match c {
            '\\' => match escape(&chars[i..]) {
                Ok(n) => i += n,
                Err(p) => {
                    note(&mut scan, p);
                    i += 1;
                }
            },
            '[' if in_class => {
                let what = if chars.get(i) == Some(&':') {
                    "a POSIX class (`[[:alpha:]]`)"
                } else {
                    "a `[` inside a character class (a nested class)"
                };
                note(&mut scan, differs(what));
            }
            '[' => {
                in_class = true;
                class_start = true;
                if chars.get(i) == Some(&'^') {
                    i += 1;
                }
            }
            ']' if in_class && !was_start => in_class = false,
            '&' | '-' | '~' if in_class && chars.get(i) == Some(&c) => {
                note(
                    &mut scan,
                    differs(&format!("the set operator `{c}{c}` in a character class")),
                );
                i += 1;
            }
            '(' if !in_class && chars.get(i) == Some(&'?') && chars.get(i + 1) != Some(&':') => {
                note(
                    &mut scan,
                    differs("an inline flag or look-around group (`(?` not followed by `:`)"),
                );
            }
            '.' if !in_class => scan.dot = true,
            _ => {}
        }
    }
    scan
}

/// Finite decimal text for a real bound, matching the real grammar.
pub(super) fn real_text(x: f64) -> String {
    format!("{x}")
}
