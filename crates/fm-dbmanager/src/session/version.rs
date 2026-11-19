//! The row version: a hash over every column, hidden ones included.

use sha2::{Digest, Sha256};

use super::cell::Cell;

/// SHA-256 over each cell as a type tag byte then its bytes, hex-encoded.
pub(super) fn version<'a>(cells: impl IntoIterator<Item = &'a Cell>) -> String {
    let mut h = Sha256::new();
    for c in cells {
        match c {
            Cell::Null => h.update([0_u8]),
            Cell::Integer(n) => {
                h.update([1_u8]);
                h.update(n.to_be_bytes());
            }
            Cell::Real(x) => {
                h.update([2_u8]);
                h.update(x.to_bits().to_be_bytes());
            }
            Cell::Text(b) => {
                h.update([3_u8]);
                h.update((b.len() as u64).to_be_bytes());
                h.update(b);
            }
            Cell::Blob(b) => {
                h.update([4_u8]);
                h.update((b.len() as u64).to_be_bytes());
                h.update(b);
            }
        }
    }
    fm_core::to_hex(&h.finalize())
}
