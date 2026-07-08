//! Bridge: implement the engine's `RuleStore` trait for `Store`.
//! Defined here because `Store` is local to this crate — orphan-rule valid.
//! The bridge stays free of inline SQL; every method forwards to an
//! existing public method on `Store`.

use pebble_core::{Folder, FolderRole, KanbanCard, Message, Result};
use pebble_rules::RuleStore;

use crate::Store;

impl RuleStore for Store {
    fn get_message(&self, id: &str) -> Result<Option<Message>> {
        Store::get_message(self, id)
    }
    fn list_message_ids_for_rules(&self) -> Result<Vec<String>> {
        Store::list_message_ids_for_rules(self)
    }
    fn list_message_ids_for_account(&self, account_id: &str) -> Result<Vec<String>> {
        Store::list_message_ids_for_account(self, account_id)
    }
    fn add_label(&self, message_id: &str, label_name: &str) -> Result<()> {
        Store::add_label(self, message_id, label_name)
    }
    fn bind_message_to_folder(&self, message_id: &str, folder_id: &str) -> Result<()> {
        Store::bind_message_to_folder(self, message_id, folder_id)
    }
    fn update_message_flags(
        &self,
        id: &str,
        is_read: Option<bool>,
        is_starred: Option<bool>,
    ) -> Result<()> {
        Store::update_message_flags(self, id, is_read, is_starred)
    }
    fn upsert_kanban_card(&self, card: &KanbanCard) -> Result<()> {
        Store::upsert_kanban_card(self, card)
    }
    fn find_folder_by_role(&self, account_id: &str, role: FolderRole) -> Result<Option<Folder>> {
        Store::find_folder_by_role(self, account_id, role)
    }
    fn find_folder_by_name(&self, account_id: &str, name: &str) -> Result<Option<Folder>> {
        Store::find_folder_by_name(self, account_id, name)
    }
    fn find_or_create_folder_by_name(
        &self,
        account_id: &str,
        name: &str,
        is_system: bool,
    ) -> Result<Folder> {
        Store::find_or_create_folder_by_name(self, account_id, name, is_system)
    }
    fn list_rules_applicable_to(&self, account_id: &str) -> Result<Vec<pebble_core::Rule>> {
        Store::list_rules_applicable_to(self, account_id)
    }
    fn list_all_rules(&self) -> Result<Vec<pebble_core::Rule>> {
        Store::list_all_rules(self)
    }
    fn list_rules_for_account_only(&self, account_id: &str) -> Result<Vec<pebble_core::Rule>> {
        Store::list_rules_for_account_only(self, account_id)
    }
}
