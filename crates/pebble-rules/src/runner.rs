//! Batch runner. Filled in Task 5 (TDD).

use crate::{Result, RuleStore};
use pebble_core::{Message, Rule};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RunStats {
    pub total: usize,
    pub matched: usize,
    pub actions_applied: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum RuleProgressEvent {
    Started {
        total: usize,
    },
    Progress {
        processed: usize,
        matched: usize,
        actions_applied: usize,
    },
    Completed {
        total: usize,
        matched: usize,
        actions_applied: usize,
        errors: usize,
    },
    Error {
        message: String,
    },
}

pub trait ProgressSink {
    fn emit(&self, ev: RuleProgressEvent);
}

#[allow(dead_code)]
pub fn run_all_rules(_store: &dyn RuleStore, _sink: Option<&dyn ProgressSink>) -> Result<RunStats> {
    Ok(RunStats::default())
}

#[allow(dead_code)]
pub fn run_single_rule(
    _rule: &Rule,
    _store: &dyn RuleStore,
    _sink: Option<&dyn ProgressSink>,
) -> Result<RunStats> {
    Ok(RunStats::default())
}

#[allow(dead_code)]
pub fn run_rules_for_new_message(_msg: &Message, _store: &dyn RuleStore) -> Result<()> {
    Ok(())
}
