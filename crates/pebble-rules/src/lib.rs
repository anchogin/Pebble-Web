//! Pebble rules engine: condition matcher + action executor + batch runner.
//!
//! This crate contains pure logic and a `RuleStore` trait. The actual
//! `impl RuleStore for pebble_store::Store` lives in pebble-store so the
//! engine crate stays free of SQLite/HTTP/sync coupling and is unit-testable
//! with mock stores.

pub mod cancel;
pub mod executor;
pub mod matcher;
pub mod model;
pub mod runner;

pub use cancel::RunControl;
pub use executor::{apply_actions, ActionOutcome};
pub use matcher::evaluate;
pub use model::{ActionType, ConditionField, ConditionOp, RuleAction, RuleCondition};
pub use runner::{
    run_all_rules, run_rules_for_new_message, run_single_rule, ProgressSink, RuleProgressEvent,
    RunStats,
};

use pebble_core::{Folder, FolderRole, KanbanCard, Message, Result, Rule};

/// Storage interface the rules engine depends on. Implemented by
/// `pebble_store::Store` in the `pebble-store` crate (orphan-rule-valid:
/// `Store` is local to that crate). Tests provide an in-memory mock impl.
pub trait RuleStore {
    fn get_message(&self, id: &str) -> Result<Option<Message>>;
    /// All message ids eligible for full-batch rule runs (excludes
    /// soft-deleted messages; sync-side responsibility for spam/drafts
    /// filtering). Returns ids only — each message is fetched via
    /// `get_message` during evaluation to avoid holding all rows in RAM.
    fn list_message_ids_for_rules(&self) -> Result<Vec<String>>;
    /// Same as above but scoped to one account.
    fn list_message_ids_for_account(&self, account_id: &str) -> Result<Vec<String>>;

    fn add_label(&self, message_id: &str, label_name: &str) -> Result<()>;
    fn bind_message_to_folder(&self, message_id: &str, folder_id: &str) -> Result<()>;
    fn update_message_flags(
        &self,
        id: &str,
        is_read: Option<bool>,
        is_starred: Option<bool>,
    ) -> Result<()>;
    fn upsert_kanban_card(&self, card: &KanbanCard) -> Result<()>;

    fn find_folder_by_role(&self, account_id: &str, role: FolderRole) -> Result<Option<Folder>>;
    fn find_folder_by_name(&self, account_id: &str, name: &str) -> Result<Option<Folder>>;
    fn find_or_create_folder_by_name(
        &self,
        account_id: &str,
        name: &str,
        is_system: bool,
    ) -> Result<Folder>;

    fn list_rules_applicable_to(&self, account_id: &str) -> Result<Vec<Rule>>;
    fn list_all_rules(&self) -> Result<Vec<Rule>>;
    fn list_rules_for_account_only(&self, account_id: &str) -> Result<Vec<Rule>>;
}
