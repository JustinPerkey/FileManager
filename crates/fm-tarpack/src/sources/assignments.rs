//! The minimal input to a build: which Windows file supplies each entry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Whether an entry's assigned file can be used right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum EntryStatus {
    /// Assigned to a file that exists.
    Ready,
    /// Assigned to a path that no longer exists or is not a file.
    Missing,
    /// No file assigned.
    Unassigned,
}

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

    /// Checks the filesystem now: is the assigned file still there?
    pub fn status(&self, id: &str) -> EntryStatus {
        match self.0.get(id) {
            None => EntryStatus::Unassigned,
            Some(p) if p.is_file() => EntryStatus::Ready,
            Some(_) => EntryStatus::Missing,
        }
    }

    /// Assignments in id order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Path)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_path()))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn map(&self) -> &BTreeMap<String, PathBuf> {
        &self.0
    }

    pub(super) fn from_map(map: BTreeMap<String, PathBuf>) -> Assignments {
        Assignments(map)
    }
}
