//! Action execution.

use std::collections::HashSet;

use crate::model::{parse_actions, ActionType};
use crate::RuleStore;
use pebble_core::{Folder, FolderRole, KanbanCard, KanbanColumn, Message, PebbleError, Result};

#[derive(Debug, Clone)]
pub struct ActionOutcome {
    pub applied: bool,
    pub skipped_reason: Option<String>,
}

pub fn apply_actions(
    actions_json: &str,
    msg: &Message,
    store: &dyn RuleStore,
    applied_action_types: &mut HashSet<ActionType>,
) -> Result<Vec<ActionOutcome>> {
    let actions = parse_actions(actions_json)
        .map_err(|e| PebbleError::Storage(format!("Invalid actions JSON: {e}")))?;
    let mut outcomes = Vec::with_capacity(actions.len());
    for a in actions {
        // Skip if same type already applied by a higher-priority rule.
        if applied_action_types.contains(&a.action_type) {
            outcomes.push(ActionOutcome {
                applied: false,
                skipped_reason: Some(format!(
                    "同类动作已由更高优先级规则执行 ({:?})",
                    a.action_type
                )),
            });
            continue;
        }
        let result: Result<()> = match a.action_type {
            ActionType::AddLabel => {
                let name = a.value.as_deref().unwrap_or("").trim().to_string();
                if name.is_empty() {
                    return Err(PebbleError::Storage(
                        "AddLabel action requires a non-empty value".into(),
                    ));
                }
                store.add_label(&msg.id, &name)
            }
            ActionType::MoveToFolder => {
                let name = a.value.as_deref().unwrap_or("").trim().to_string();
                if name.is_empty() {
                    return Err(PebbleError::Storage(
                        "MoveToFolder action requires a non-empty value".into(),
                    ));
                }
                let folder = resolve_folder(&name, &msg.account_id, store)?;
                store.bind_message_to_folder(&msg.id, &folder.id)
            }
            ActionType::MarkRead => store.update_message_flags(&msg.id, Some(true), None),
            ActionType::Archive => {
                let folder = resolve_archive(&msg.account_id, store)?;
                store.bind_message_to_folder(&msg.id, &folder.id)
            }
            ActionType::SetKanbanColumn => {
                let col_name = a.value.as_deref().unwrap_or("todo");
                let column = match col_name {
                    "waiting" => KanbanColumn::Waiting,
                    "done" => KanbanColumn::Done,
                    _ => KanbanColumn::Todo,
                };
                let now = pebble_core::now_timestamp();
                store.upsert_kanban_card(&KanbanCard {
                    message_id: msg.id.clone(),
                    column,
                    position: 0,
                    created_at: now,
                    updated_at: now,
                })
            }
        };
        match result {
            Ok(()) => {
                applied_action_types.insert(a.action_type);
                outcomes.push(ActionOutcome {
                    applied: true,
                    skipped_reason: None,
                });
            }
            Err(e) => return Err(e),
        }
    }
    Ok(outcomes)
}

/// First match known role names; else find_or_create by name.
fn resolve_folder(name: &str, account_id: &str, store: &dyn RuleStore) -> Result<Folder> {
    let role = match name {
        "inbox" => Some(FolderRole::Inbox),
        "sent" => Some(FolderRole::Sent),
        "drafts" => Some(FolderRole::Drafts),
        "trash" => Some(FolderRole::Trash),
        "spam" => Some(FolderRole::Spam),
        "archive" => Some(FolderRole::Archive),
        _ => None,
    };
    if let Some(role) = role {
        if let Ok(Some(f)) = store.find_folder_by_role(account_id, role) {
            return Ok(f);
        }
    }
    let f = store.find_or_create_folder_by_name(account_id, name, false)?;
    Ok(f)
}

/// Archive: try role folder; if missing, auto-create an `Archive` system folder.
fn resolve_archive(account_id: &str, store: &dyn RuleStore) -> Result<Folder> {
    if let Ok(Some(f)) = store.find_folder_by_role(account_id, FolderRole::Archive) {
        return Ok(f);
    }
    store.find_or_create_folder_by_name(account_id, "Archive", true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pebble_core::{EmailAddress, FolderType, Rule};
    use std::sync::{Arc, Mutex};

    /// Mock RuleStore that records calls and can be configured with canned responses.
    #[derive(Default, Clone)]
    struct MockStore {
        add_label_calls: Arc<Mutex<Vec<(String, String)>>>,
        bind_calls: Arc<Mutex<Vec<(String, String)>>>,
        flag_calls: Arc<Mutex<Vec<(String, Option<bool>, Option<bool>)>>>,
        kanban_calls: Arc<Mutex<Vec<KanbanCard>>>,
        archive_folder: Option<Folder>,
        existing_folders: Arc<Mutex<Vec<Folder>>>,
        created_folders: Arc<Mutex<Vec<Folder>>>,
        rules: Arc<Mutex<Vec<Rule>>>,
    }

    impl RuleStore for MockStore {
        fn get_message(&self, _id: &str) -> Result<Option<Message>> {
            Ok(None)
        }
        fn list_message_ids_for_rules(&self) -> Result<Vec<String>> {
            Ok(Vec::new())
        }
        fn list_message_ids_for_account(&self, _a: &str) -> Result<Vec<String>> {
            Ok(Vec::new())
        }
        fn add_label(&self, mid: &str, name: &str) -> Result<()> {
            self.add_label_calls
                .lock()
                .unwrap()
                .push((mid.into(), name.into()));
            Ok(())
        }
        fn bind_message_to_folder(&self, mid: &str, fid: &str) -> Result<()> {
            self.bind_calls
                .lock()
                .unwrap()
                .push((mid.into(), fid.into()));
            Ok(())
        }
        fn update_message_flags(&self, id: &str, r: Option<bool>, s: Option<bool>) -> Result<()> {
            self.flag_calls.lock().unwrap().push((id.into(), r, s));
            Ok(())
        }
        fn upsert_kanban_card(&self, card: &KanbanCard) -> Result<()> {
            self.kanban_calls.lock().unwrap().push(card.clone());
            Ok(())
        }
        fn find_folder_by_role(
            &self,
            _account_id: &str,
            role: FolderRole,
        ) -> Result<Option<Folder>> {
            if role == FolderRole::Archive {
                Ok(self.archive_folder.clone())
            } else {
                Ok(None)
            }
        }
        fn find_folder_by_name(&self, _account_id: &str, name: &str) -> Result<Option<Folder>> {
            Ok(self
                .existing_folders
                .lock()
                .unwrap()
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(name))
                .cloned())
        }
        fn find_or_create_folder_by_name(
            &self,
            account_id: &str,
            name: &str,
            is_system: bool,
        ) -> Result<Folder> {
            // Idempotent: if name exists, return it.
            if let Some(existing) = self.find_folder_by_name(account_id, name)? {
                return Ok(existing);
            }
            let f = Folder {
                id: format!("new-{}", name),
                account_id: account_id.into(),
                remote_id: format!("local-{}", name),
                name: name.into(),
                folder_type: FolderType::Folder,
                role: if is_system && name.eq_ignore_ascii_case("Archive") {
                    Some(FolderRole::Archive)
                } else {
                    None
                },
                parent_id: None,
                color: None,
                is_system,
                server_linked: false,
                sort_order: 1000,
            };
            self.created_folders.lock().unwrap().push(f.clone());
            Ok(f)
        }
        fn list_rules_applicable_to(&self, _a: &str) -> Result<Vec<Rule>> {
            Ok(self.rules.lock().unwrap().clone())
        }
        fn list_all_rules(&self) -> Result<Vec<Rule>> {
            Ok(self.rules.lock().unwrap().clone())
        }
        fn list_rules_for_account_only(&self, _a: &str) -> Result<Vec<Rule>> {
            Ok(self
                .rules
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.account_id.is_some())
                .cloned()
                .collect())
        }
    }

    fn mk_msg() -> Message {
        Message {
            id: "msg1".into(),
            account_id: "acc1".into(),
            remote_id: "u1".into(),
            message_id_header: None,
            in_reply_to: None,
            references_header: None,
            thread_id: None,
            subject: "s".into(),
            snippet: String::new(),
            from_address: "a@b.com".into(),
            from_name: String::new(),
            to_list: vec![EmailAddress {
                name: None,
                address: "x@y.com".into(),
            }],
            cc_list: vec![],
            bcc_list: vec![],
            body_text: String::new(),
            body_html_raw: String::new(),
            has_attachments: false,
            is_read: false,
            is_starred: false,
            is_draft: false,
            date: 0,
            remote_version: None,
            is_deleted: false,
            deleted_at: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn add_label_applies_and_records() {
        let s = MockStore::default();
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let out = apply_actions(
            r#"[{"type":"AddLabel","value":"work"}]"#,
            &msg,
            &s,
            &mut applied,
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].applied);
        assert!(applied.contains(&ActionType::AddLabel));
        assert_eq!(
            s.add_label_calls.lock().unwrap().clone(),
            vec![("msg1".into(), "work".into())]
        );
    }

    #[test]
    fn duplicate_add_label_is_skipped() {
        let s = MockStore::default();
        let msg = mk_msg();
        let mut applied = HashSet::new();
        applied.insert(ActionType::AddLabel);
        let out = apply_actions(
            r#"[{"type":"AddLabel","value":"work"}]"#,
            &msg,
            &s,
            &mut applied,
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert!(!out[0].applied);
        assert!(out[0].skipped_reason.is_some());
        assert!(s.add_label_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn mark_read_calls_flags() {
        let s = MockStore::default();
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let _ = apply_actions(r#"[{"type":"MarkRead"}]"#, &msg, &s, &mut applied).unwrap();
        assert_eq!(
            s.flag_calls.lock().unwrap().clone(),
            vec![("msg1".into(), Some(true), None)]
        );
    }

    #[test]
    fn archive_uses_existing_role_folder() {
        let mut s = MockStore::default();
        s.archive_folder = Some(Folder {
            id: "arch1".into(),
            account_id: "acc1".into(),
            remote_id: "Arch".into(),
            name: "Archive".into(),
            folder_type: FolderType::Folder,
            role: Some(FolderRole::Archive),
            parent_id: None,
            color: None,
            is_system: true,
            server_linked: false,
            sort_order: 0,
        });
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let _ = apply_actions(r#"[{"type":"Archive"}]"#, &msg, &s, &mut applied).unwrap();
        assert_eq!(
            s.bind_calls.lock().unwrap().clone(),
            vec![("msg1".into(), "arch1".into())]
        );
        assert!(s.created_folders.lock().unwrap().is_empty());
    }

    #[test]
    fn archive_creates_local_archive_when_missing() {
        let s = MockStore::default();
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let _ = apply_actions(r#"[{"type":"Archive"}]"#, &msg, &s, &mut applied).unwrap();
        assert_eq!(s.created_folders.lock().unwrap().len(), 1);
        let f = &s.created_folders.lock().unwrap()[0];
        assert_eq!(f.name, "Archive");
        assert!(f.is_system);
        assert_eq!(f.role, Some(FolderRole::Archive));
    }

    #[test]
    fn move_to_folder_existing_name_resolves() {
        let s = MockStore::default();
        s.existing_folders.lock().unwrap().push(Folder {
            id: "f1".into(),
            account_id: "acc1".into(),
            remote_id: "Work".into(),
            name: "Work".into(),
            folder_type: FolderType::Folder,
            role: None,
            parent_id: None,
            color: None,
            is_system: false,
            server_linked: false,
            sort_order: 5,
        });
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let _ = apply_actions(
            r#"[{"type":"MoveToFolder","value":"Work"}]"#,
            &msg,
            &s,
            &mut applied,
        )
        .unwrap();
        assert_eq!(
            s.bind_calls.lock().unwrap().clone(),
            vec![("msg1".into(), "f1".into())]
        );
        assert!(s.created_folders.lock().unwrap().is_empty());
    }

    #[test]
    fn move_to_folder_unknown_name_creates_local() {
        let s = MockStore::default();
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let _ = apply_actions(
            r#"[{"type":"MoveToFolder","value":"Projects"}]"#,
            &msg,
            &s,
            &mut applied,
        )
        .unwrap();
        assert_eq!(s.created_folders.lock().unwrap().len(), 1);
        let f = &s.created_folders.lock().unwrap()[0];
        assert_eq!(f.name, "Projects");
        assert_eq!(f.remote_id, "local-Projects");
        assert!(!f.is_system);
        assert_eq!(s.bind_calls.lock().unwrap()[0].1, f.id);
    }

    #[test]
    fn move_to_folder_role_name_uses_role_lookup() {
        let mut s = MockStore::default();
        s.archive_folder = Some(Folder {
            id: "arch".into(),
            account_id: "acc1".into(),
            remote_id: "Archive".into(),
            name: "Archive".into(),
            folder_type: FolderType::Folder,
            role: Some(FolderRole::Archive),
            parent_id: None,
            color: None,
            is_system: true,
            server_linked: false,
            sort_order: 0,
        });
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let _ = apply_actions(
            r#"[{"type":"MoveToFolder","value":"archive"}]"#,
            &msg,
            &s,
            &mut applied,
        )
        .unwrap();
        assert_eq!(s.bind_calls.lock().unwrap()[0].1, "arch");
        assert!(s.created_folders.lock().unwrap().is_empty());
    }

    #[test]
    fn set_kanban_column_default_todo() {
        let s = MockStore::default();
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let _ = apply_actions(
            r#"[{"type":"SetKanbanColumn","value":"todo"}]"#,
            &msg,
            &s,
            &mut applied,
        )
        .unwrap();
        assert_eq!(s.kanban_calls.lock().unwrap()[0].column, KanbanColumn::Todo);
    }
    #[test]
    fn set_kanban_column_done() {
        let s = MockStore::default();
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let _ = apply_actions(
            r#"[{"type":"SetKanbanColumn","value":"done"}]"#,
            &msg,
            &s,
            &mut applied,
        )
        .unwrap();
        assert_eq!(s.kanban_calls.lock().unwrap()[0].column, KanbanColumn::Done);
    }

    #[test]
    fn empty_add_label_value_errors() {
        let s = MockStore::default();
        let msg = mk_msg();
        let mut applied = HashSet::new();
        let res = apply_actions(
            r#"[{"type":"AddLabel","value":""}]"#,
            &msg,
            &s,
            &mut applied,
        );
        assert!(res.is_err());
    }
}
