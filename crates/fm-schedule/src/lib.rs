//! Schedule Creator domain library: reads a schedule text file and adds it to
//! an existing XML document. Depends on nothing from tauri.
//!
//! Two hooks do the real work, and both are stubs that return
//! `NotImplemented` until they are written:
//!
//! - [`parse`] turns the text file's contents into a [`Schedule`];
//! - [`merge`] adds a [`Schedule`] to the XML document's text.
//!
//! Both run only when the user confirms the update. Everything around them
//! (choosing files, confirming, the backup, and the crash-safe write) is already wired up in the desktop shell,
//! so filling in these two functions, and the [`Schedule`] model they share,
//! is all the tool needs.

pub mod merge;
pub mod model;
pub mod parse;

pub use merge::{merge, MergeError, MergeOutcome};
pub use model::Schedule;
pub use parse::{parse, ParseError};
