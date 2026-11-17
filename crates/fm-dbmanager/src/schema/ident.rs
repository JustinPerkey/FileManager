//! `Ident`: the only way a table or column name reaches SQL text.

use std::fmt;

/// A table or column name read from the database.
///
/// There is deliberately no public constructor, no `From<&str>` and no
/// `Deserialize`: an `Ident` can only come from introspection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident(String);

impl Ident {
    pub(crate) fn new(name: String) -> Ident {
        Ident(name)
    }

    /// The raw name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The name as a quoted SQL identifier: wrapped in `"`, embedded `"`
    /// doubled.
    #[must_use]
    pub fn quoted(&self) -> String {
        format!("\"{}\"", self.0.replace('"', "\"\""))
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
