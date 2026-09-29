//! Octal permission strings.

/// Parses an octal string of 1 to 4 digits.
pub fn parse_mode(s: &str) -> Result<u32, String> {
    if s.is_empty() || s.len() > 4 {
        return Err(format!(
            "mode `{s}` must be an octal string of 1 to 4 digits"
        ));
    }
    if !s.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return Err(format!("mode `{s}` is not an octal string"));
    }
    // 1 to 4 octal digits cannot exceed 0o7777 and cannot fail to parse.
    Ok(s.bytes().fold(0, |acc, b| acc * 8 + u32::from(b - b'0')))
}

/// Renders the low 12 bits as `rwxr-xr-x`, with `s`/`S` and `t`/`T` for the
/// setuid, setgid and sticky bits.
pub fn symbolic(mode: u32) -> String {
    let mut out = String::with_capacity(9);
    for (shift, special_bit, special_on, special_off) in [
        (6, 0o4000, 's', 'S'),
        (3, 0o2000, 's', 'S'),
        (0, 0o1000, 't', 'T'),
    ] {
        let bits = (mode >> shift) & 7;
        out.push(if bits & 4 != 0 { 'r' } else { '-' });
        out.push(if bits & 2 != 0 { 'w' } else { '-' });
        out.push(match (mode & special_bit != 0, bits & 1 != 0) {
            (true, true) => special_on,
            (true, false) => special_off,
            (false, true) => 'x',
            (false, false) => '-',
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parses_octal_strings() {
        for (s, want) in [
            ("0", 0),
            ("644", 0o644),
            ("0644", 0o644),
            ("0755", 0o755),
            ("7777", 0o7777),
            ("4755", 0o4755),
        ] {
            assert_eq!(parse_mode(s), Ok(want), "{s}");
        }
        for s in ["", "00644", "0898", "rwx", "-1", "+64", " 64", "0x1"] {
            assert!(parse_mode(s).is_err(), "{s}");
        }
    }

    #[test]
    fn symbolic_renders() {
        assert_eq!(symbolic(0o755), "rwxr-xr-x");
        assert_eq!(symbolic(0o640), "rw-r-----");
        assert_eq!(symbolic(0o4755), "rwsr-xr-x");
        assert_eq!(symbolic(0o1777), "rwxrwxrwt");
        assert_eq!(symbolic(0o2644), "rw-r-Sr--");
    }
}
