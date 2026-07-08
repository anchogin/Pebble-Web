//! Batch runner.

use std::collections::HashSet;

use crate::executor::apply_actions;
use crate::matcher::evaluate;
use crate::model::ActionType;
use crate::{RuleStore, RunControl};
use pebble_core::{Message, Result, Rule};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RunStats {
    pub total: usize,
    pub matched: usize,
    pub actions_applied: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "phase")]
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
        processed: usize,
        matched: usize,
        actions_applied: usize,
        errors: usize,
    },
    Cancelled {
        total: usize,
        processed: usize,
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

/// Run all enabled rules against every non-deleted message in the store.
pub fn run_all_rules(
    store: &dyn RuleStore,
    sink: Option<&dyn ProgressSink>,
    control: Option<&RunControl>,
) -> Result<RunStats> {
    let rules = store.list_all_rules()?;
    let enabled: Vec<Rule> = rules.into_iter().filter(|r| r.is_enabled).collect();
    let ids = store.list_message_ids_for_rules()?;
    run_batch(&enabled, &ids, store, sink, control)
}

/// Run a single rule against every message (regardless of enabled state,
/// because the user explicitly invoked it).
pub fn run_single_rule(
    rule: &Rule,
    store: &dyn RuleStore,
    sink: Option<&dyn ProgressSink>,
    control: Option<&RunControl>,
) -> Result<RunStats> {
    let ids = if let Some(ref aid) = rule.account_id {
        store.list_message_ids_for_account(aid)?
    } else {
        store.list_message_ids_for_rules()?
    };
    run_batch(std::slice::from_ref(rule), &ids, store, sink, control)
}

/// Run all rules applicable to a newly-inserted message. Called by `sync.rs`
/// right after `store.insert_message`. No progress events (fast, single msg).
pub fn run_rules_for_new_message(msg: &Message, store: &dyn RuleStore) -> Result<()> {
    let mut applied: HashSet<ActionType> = HashSet::new();
    let rules = store.list_rules_applicable_to(&msg.account_id)?;
    for rule in &rules {
        if !applicable_to_message(rule, msg) {
            continue;
        }
        if evaluate(&rule.conditions, msg)? {
            let outcomes = apply_actions(&rule.actions, msg, store, &mut applied)?;
            for o in &outcomes {
                if o.applied {
                    tracing::debug!(
                        rule_id = %rule.id, message_id = %msg.id,
                        "applied action"
                    );
                }
            }
        }
    }
    Ok(())
}

fn run_batch(
    rules: &[Rule],
    ids: &[String],
    store: &dyn RuleStore,
    sink: Option<&dyn ProgressSink>,
    control: Option<&RunControl>,
) -> Result<RunStats> {
    let total = ids.len();
    tracing::info!(
        rule_count = rules.len(),
        total,
        "rules runner: batch started"
    );
    let mut stats = RunStats {
        total,
        ..Default::default()
    };
    if let Some(s) = sink {
        s.emit(RuleProgressEvent::Started { total });
    }
    for (idx, id) in ids.iter().enumerate() {
        if control.is_some_and(RunControl::is_cancelled) {
            tracing::info!(
                processed = idx,
                total,
                matched = stats.matched,
                actions_applied = stats.actions_applied,
                errors = stats.errors,
                "rules runner: batch cancelled"
            );
            if let Some(s) = sink {
                s.emit(RuleProgressEvent::Cancelled {
                    total,
                    processed: idx,
                    matched: stats.matched,
                    actions_applied: stats.actions_applied,
                    errors: stats.errors,
                });
            }
            return Ok(stats);
        }
        tracing::debug!(processed = idx, total, message_id = %id, "rules runner: loading message");
        let msg = match store.get_message(id) {
            Ok(Some(m)) => m,
            Ok(None) => continue,
            Err(e) => {
                stats.errors += 1;
                tracing::warn!(message_id = %id, error = %e, "rule run: get_message failed");
                continue;
            }
        };
        let mut applied: HashSet<ActionType> = HashSet::new();
        let mut matched = false;
        for rule in rules {
            if !applicable_to_message(rule, &msg) {
                continue;
            }
            match evaluate(&rule.conditions, &msg) {
                Ok(true) => {
                    matched = true;
                    match apply_actions(&rule.actions, &msg, store, &mut applied) {
                        Ok(outcomes) => {
                            stats.actions_applied +=
                                outcomes.into_iter().filter(|o| o.applied).count()
                        }
                        Err(e) => {
                            stats.errors += 1;
                            tracing::warn!(rule_id = %rule.id, message_id = %msg.id, error = %e, "apply_actions failed");
                        }
                    }
                }
                Ok(false) => {}
                Err(e) => {
                    stats.errors += 1;
                    tracing::warn!(rule_id = %rule.id, message_id = %msg.id, error = %e, "evaluate failed");
                }
            }
        }
        if matched {
            stats.matched += 1;
        }
        if let Some(s) = sink {
            s.emit(RuleProgressEvent::Progress {
                processed: idx + 1,
                matched: stats.matched,
                actions_applied: stats.actions_applied,
            });
        }
        if (idx + 1) % 100 == 0 || idx + 1 == total {
            tracing::info!(
                processed = idx + 1,
                total,
                matched = stats.matched,
                actions_applied = stats.actions_applied,
                errors = stats.errors,
                message_id = %id,
                "rules runner: progress checkpoint"
            );
        }
    }
    tracing::info!(
        total,
        matched = stats.matched,
        actions_applied = stats.actions_applied,
        errors = stats.errors,
        "rules runner: batch completed"
    );
    if let Some(s) = sink {
        s.emit(RuleProgressEvent::Completed {
            total,
            processed: total,
            matched: stats.matched,
            actions_applied: stats.actions_applied,
            errors: stats.errors,
        });
    }
    Ok(stats)
}

/// `run_all_rules` already filtered enabled rules; `run_single_rule` ignores enabled.
/// Filter rule-to-account scope: if the rule's `account_id` exists, the
/// message's `account_id` must equal it.
fn applicable_to_message(rule: &Rule, msg: &Message) -> bool {
    match &rule.account_id {
        Some(aid) => aid == &msg.account_id,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pebble_core::{EmailAddress, Folder, FolderRole, FolderType, KanbanCard};
    use std::sync::{Arc, Mutex};

    #[derive(Default, Clone)]
    struct MockStore {
        messages: Vec<Message>,
        add_label_calls: Arc<Mutex<Vec<(String, String)>>>,
        flag_calls: Arc<Mutex<Vec<(String, Option<bool>, Option<bool>)>>>,
        rules: Vec<Rule>,
    }

    impl RuleStore for MockStore {
        fn get_message(&self, id: &str) -> Result<Option<Message>> {
            Ok(self.messages.iter().find(|m| m.id == id).cloned())
        }
        fn list_message_ids_for_rules(&self) -> Result<Vec<String>> {
            Ok(self.messages.iter().map(|m| m.id.clone()).collect())
        }
        fn list_message_ids_for_account(&self, account_id: &str) -> Result<Vec<String>> {
            Ok(self
                .messages
                .iter()
                .filter(|m| m.account_id == account_id)
                .map(|m| m.id.clone())
                .collect())
        }
        fn add_label(&self, mid: &str, name: &str) -> Result<()> {
            self.add_label_calls
                .lock()
                .unwrap()
                .push((mid.into(), name.into()));
            Ok(())
        }
        fn bind_message_to_folder(&self, _mid: &str, _fid: &str) -> Result<()> {
            Ok(())
        }
        fn update_message_flags(&self, id: &str, r: Option<bool>, s: Option<bool>) -> Result<()> {
            self.flag_calls.lock().unwrap().push((id.into(), r, s));
            Ok(())
        }
        fn upsert_kanban_card(&self, _card: &KanbanCard) -> Result<()> {
            Ok(())
        }
        fn find_folder_by_role(&self, _a: &str, _r: FolderRole) -> Result<Option<Folder>> {
            Ok(None)
        }
        fn find_folder_by_name(&self, _a: &str, _n: &str) -> Result<Option<Folder>> {
            Ok(None)
        }
        fn find_or_create_folder_by_name(
            &self,
            account_id: &str,
            name: &str,
            _sys: bool,
        ) -> Result<Folder> {
            Ok(Folder {
                id: format!("local-{}", name),
                account_id: account_id.into(),
                remote_id: format!("local-{}", name),
                name: name.into(),
                folder_type: FolderType::Folder,
                role: None,
                parent_id: None,
                color: None,
                is_system: false,
                server_linked: false,
                sort_order: 1000,
            })
        }
        fn list_rules_applicable_to(&self, account_id: &str) -> Result<Vec<Rule>> {
            Ok(self
                .rules
                .iter()
                .filter(|r| r.account_id.is_none() || r.account_id.as_deref() == Some(account_id))
                .cloned()
                .collect())
        }
        fn list_all_rules(&self) -> Result<Vec<Rule>> {
            Ok(self.rules.clone())
        }
        fn list_rules_for_account_only(&self, account_id: &str) -> Result<Vec<Rule>> {
            Ok(self
                .rules
                .iter()
                .filter(|r| r.account_id.as_deref() == Some(account_id))
                .cloned()
                .collect())
        }
    }

    fn mk_msg(id: &str, account_id: &str, from: &str) -> Message {
        Message {
            id: id.into(),
            account_id: account_id.into(),
            remote_id: id.into(),
            message_id_header: None,
            in_reply_to: None,
            references_header: None,
            thread_id: None,
            subject: "s".into(),
            snippet: String::new(),
            from_address: from.into(),
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

    fn mk_rule(
        id: &str,
        cond_contains_from: &str,
        action_label: &str,
        account_id: Option<&str>,
    ) -> Rule {
        Rule {
            id: id.into(), name: id.into(), priority: 1,
            conditions: serde_json::json!({"operator":"and","conditions":[{"field":"from","op":"contains","value":cond_contains_from}]}).to_string(),
            actions: serde_json::json!([{"type":"AddLabel","value":action_label}]).to_string(),
            is_enabled: true, account_id: account_id.map(str::to_string),
            created_at: 0, updated_at: 0,
        }
    }

    fn mk_rule_markread(id: &str, cond_contains_from: &str, account_id: Option<&str>) -> Rule {
        Rule {
            id: id.into(), name: id.into(), priority: 1,
            conditions: serde_json::json!({"operator":"and","conditions":[{"field":"from","op":"contains","value":cond_contains_from}]}).to_string(),
            actions: r#"[{"type":"MarkRead"}]"#.to_string(),
            is_enabled: true, account_id: account_id.map(str::to_string),
            created_at: 0, updated_at: 0,
        }
    }

    #[test]
    fn run_all_applies_global_rule_to_matching_message() {
        let mut s = MockStore::default();
        s.messages = vec![mk_msg("m1", "acc1", "boss@x.com")];
        s.rules = vec![mk_rule("r1", "boss", "ignored", None)];
        let stats = run_all_rules(&s, None, None).unwrap();
        assert_eq!(stats.matched, 1);
        assert!(s.add_label_calls.lock().unwrap()[0].1 == "ignored");
    }

    #[test]
    fn run_all_skips_rule_scoped_to_other_account() {
        let mut s = MockStore::default();
        s.messages = vec![mk_msg("m1", "acc1", "boss@x.com")];
        s.rules = vec![mk_rule("r1", "boss", "wglabel", Some("acc2"))];
        let stats = run_all_rules(&s, None, None).unwrap();
        assert_eq!(stats.matched, 0);
        assert!(s.add_label_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn run_all_one_match_one_miss_total_correct() {
        let mut s = MockStore::default();
        s.messages = vec![
            mk_msg("m1", "acc1", "boss@x.com"),
            mk_msg("m2", "acc1", "miss@x.com"),
        ];
        s.rules = vec![mk_rule("r1", "boss", "bo", None)];
        let stats = run_all_rules(&s, None, None).unwrap();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.matched, 1);
    }

    #[test]
    fn run_single_rule_runs_otherwise_disabled() {
        let mut s = MockStore::default();
        s.messages = vec![mk_msg("m1", "acc1", "boss@x.com")];
        let mut r = mk_rule("r1", "boss", "lab", None);
        r.is_enabled = false;
        s.rules = vec![];
        let stats = run_single_rule(&r, &s, None, None).unwrap();
        assert_eq!(stats.matched, 1);
    }

    #[test]
    fn run_rules_for_new_message_skips_other_account_rule() {
        let mut s = MockStore::default();
        let msg = mk_msg("m1", "acc1", "boss@x.com");
        // global rule uses MarkRead (a different ActionType) so its action does
        // not collide with this_acct's AddLabel; the engine dedups by ActionType,
        // so two AddLabel rules would have only the first fire.
        s.rules = vec![
            mk_rule_markread("global", "boss", None),
            mk_rule("other_acct", "boss", "o", Some("acc2")),
            mk_rule("this_acct", "boss", "t", Some("acc1")),
        ];
        run_rules_for_new_message(&msg, &s).unwrap();
        let labels: Vec<String> = s
            .add_label_calls
            .lock()
            .unwrap()
            .iter()
            .map(|(_, n)| n.clone())
            .collect();
        assert!(labels.contains(&"t".to_string()));
        assert!(!labels.contains(&"o".to_string()));
        let flags = s.flag_calls.lock().unwrap().clone();
        assert_eq!(flags, vec![("m1".to_string(), Some(true), None)]);
    }

    #[test]
    fn emits_started_and_completed_no_sink_safe() {
        let s = MockStore::default();
        run_all_rules(&s, None, None).unwrap(); // no panic
    }

    #[derive(Clone)]
    struct CancellingSink {
        control: RunControl,
        events: Arc<Mutex<Vec<RuleProgressEvent>>>,
    }

    impl ProgressSink for CancellingSink {
        fn emit(&self, ev: RuleProgressEvent) {
            if let RuleProgressEvent::Progress { processed, .. } = ev {
                if processed == 10 {
                    self.control.cancel();
                }
            }
            self.events.lock().unwrap().push(ev);
        }
    }

    #[test]
    fn run_all_emits_cancelled_after_control_cancelled() {
        let mut s = MockStore::default();
        s.messages = (0..50)
            .map(|idx| mk_msg(&format!("m{idx}"), "acc1", "boss@x.com"))
            .collect();
        s.rules = vec![mk_rule("r1", "boss", "ignored", None)];
        let control = RunControl::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = CancellingSink {
            control: control.clone(),
            events: events.clone(),
        };

        let stats = run_all_rules(&s, Some(&sink), Some(&control)).unwrap();

        assert!(stats.total == 50);
        assert!(stats.matched < 50);
        let recorded = events.lock().unwrap();
        assert!(matches!(
            recorded.first(),
            Some(RuleProgressEvent::Started { total: 50 })
        ));
        assert!(matches!(
            recorded.last(),
            Some(RuleProgressEvent::Cancelled { processed: 10, .. })
        ));
    }
}
