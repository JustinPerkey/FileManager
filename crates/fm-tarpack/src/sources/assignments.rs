//! The minimal input to a build: which Windows file supplies each entry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Entry id to the Windows file that supplies its bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assignments(BTreeMap<String, PathBuf>);

impl Assignments {
    pub fn new() -> Assignments {
        Assignments(BTreeMap::new())
    }

    /// Assigns `source` to `id`, returning the previous assignment if any.
    pub fn insert(&mut self, id: impl Into<String>, source: impl Into<PathBuf>) -> Option<PathBuf> {
        self.0.insert(id.into(), source.into())
    }

    pub fn get(&self, id: &str) -> Option<&Path> {
        self.0.get(id).map(PathBuf::as_path)
    }

    pub fn remove(&mut self, id: &str) -> Option<PathBuf> {
        self.0.remove(id)
    }
}
