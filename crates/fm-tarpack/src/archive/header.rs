//! GNU tar headers with absolute names, and GNU long-name records.
//!
//! `tar::Header::set_path` and `Builder::append_data` reject absolute paths, so
//! the name bytes are written into the header directly. The ustar
//! `prefix`/`name` split is not used: GNU headers reuse those bytes.

use std::io::{self, Cursor, Read, Write};

use tar::{Builder, EntryType, Header};

use crate::manifest::Owner;

pub(crate) const BLOCK: u64 = 512;
/// The end-of-archive marker: two zero blocks.
pub(crate) const END_BLOCKS: u64 = 2 * BLOCK;
/// Names this long or longer get a long-name record (there is no room for the
/// terminating NUL in the 100-byte field), as GNU tar does.
const NAME_FIELD: usize = 100;

fn padded(n: u64) -> u64 {
    n.div_ceil(BLOCK) * BLOCK
}

fn is_long(name_len: usize) -> bool {
    name_len >= NAME_FIELD
}

/// Bytes a record occupies in the tar stream: an optional long-name record,
/// the header, and the padded data.
pub(crate) fn record_size(name_len: usize, data_len: u64) -> u64 {
    let long = if is_long(name_len) {
        BLOCK + padded(name_len as u64 + 1)
    } else {
        0
    };
    long + BLOCK + padded(data_len)
}

/// Bytes before the data of a record: the long-name record and the header.
pub(crate) fn preamble_size(name_len: usize) -> u64 {
    record_size(name_len, 0)
}

pub(crate) struct Meta<'a> {
    pub kind: EntryType,
    pub mode: u32,
    pub owner: &'a Owner,
    pub mtime: u64,
    pub size: u64,
}

fn raw_header(name: &[u8], kind: EntryType) -> Header {
    let mut h = Header::new_gnu();
    h.set_entry_type(kind);
    let field = &mut h.as_old_mut().name;
    field.fill(0);
    let n = name.len().min(NAME_FIELD - 1);
    field[..n].copy_from_slice(&name[..n]);
    h
}

/// Appends one entry named `name` (raw bytes, absolute), preceded by a
/// long-name record when the name needs one.
pub(crate) fn append_entry<W: Write>(
    builder: &mut Builder<W>,
    name: &[u8],
    meta: &Meta<'_>,
    data: &mut dyn Read,
) -> io::Result<()> {
    if is_long(name.len()) {
        let mut long = raw_header(b"././@LongLink", EntryType::GNULongName);
        long.set_mode(0o644);
        long.set_uid(0);
        long.set_gid(0);
        long.set_mtime(0);
        long.set_size(name.len() as u64 + 1);
        long.set_cksum();
        let mut bytes = name.to_vec();
        bytes.push(0);
        builder.append(&long, Cursor::new(bytes))?;
    }
    let mut h = raw_header(name, meta.kind);
    h.set_mode(meta.mode);
    h.set_uid(u64::from(meta.owner.uid));
    h.set_gid(u64::from(meta.owner.gid));
    h.set_username(&meta.owner.uname)?;
    h.set_groupname(&meta.owner.gname)?;
    h.set_mtime(meta.mtime);
    h.set_size(meta.size);
    h.set_cksum();
    builder.append(&h, data)
}
