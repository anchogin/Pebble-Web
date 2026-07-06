//! Condition evaluation. Filled in Task 3 (TDD).

use crate::model::ConditionsDoc;
use pebble_core::{Message, Result};

pub fn evaluate(_conditions: &str, _msg: &Message) -> Result<bool> {
    // Filled in Task 3
    Ok(false)
}

#[allow(dead_code)]
pub(crate) fn evaluate_predicate(_doc: &ConditionsDoc, _msg: &Message) -> bool {
    false
}
