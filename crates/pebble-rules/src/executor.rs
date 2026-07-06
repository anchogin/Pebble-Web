//! Action execution. Filled in Task 4 (TDD).

use crate::{model::ActionType, RuleStore};
use pebble_core::{Message, Result};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ActionOutcome {
    pub applied: bool,
    pub skipped_reason: Option<String>,
}

#[allow(dead_code)]
pub fn apply_actions(
    _actions: &str,
    _msg: &Message,
    _store: &dyn RuleStore,
    _applied_action_types: &mut HashSet<ActionType>,
) -> Result<Vec<ActionOutcome>> {
    Ok(Vec::new())
}
