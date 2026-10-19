//! Lowercase hex encoding, for digests shown to the user.

use std::fmt::Write;

/// `bytes` as lowercase hex, two digits per byte.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            // Writing to a `String` cannot fail.
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_lowercase_two_digits_per_byte() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xab, 0xff]), "000fabff");
        assert_eq!(to_hex(&[]), "");
    }
}
