//! Window-level behaviour: the root view, developer mode, drop routing, the
//! close guard, shared text.

pub mod dev_mode;
pub mod dirs;
pub mod drop;
pub mod guard;
pub mod path;
pub mod probe;
pub mod shell;
pub mod text;
pub mod view;

#[cfg(test)]
pub(crate) mod tests;
