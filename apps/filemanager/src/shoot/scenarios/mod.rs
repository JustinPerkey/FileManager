//! The scenario list. This directory belongs to the ui-implementer, who adds
//! one scenario per manual screenshot (and removes or replaces the sample).
//! Order here is the order the harness runs and lists them.

use super::{Scenario, Step};
use crate::tools::ToolId;

/// Every scenario, in order.
#[must_use]
pub fn all() -> Vec<Scenario> {
    vec![shell_sample()]
}

/// The placeholder shell with the Schedule Creator selected. Proves the
/// harness end to end; the ui-implementer replaces or removes it.
fn shell_sample() -> Scenario {
    Scenario {
        name: "shell-sample",
        size: (1000.0, 700.0),
        setup: |_| {},
        steps: &[Step::SelectTool(ToolId::Schedule), Step::WaitIdle],
        clip: None,
    }
}
