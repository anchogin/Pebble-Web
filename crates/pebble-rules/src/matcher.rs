//! Condition evaluation.

use crate::model::{ConditionField, ConditionOp, ConditionsDoc};
use pebble_core::{EmailAddress, Message, PebbleError, Result};

/// Parse conditions JSON + evaluate all predicates against `msg`.
/// `operator="and"` only; `"or"` → Err.
pub fn evaluate(conditions_json: &str, msg: &Message) -> Result<bool> {
    let doc: ConditionsDoc = crate::model::parse_conditions(conditions_json)
        .map_err(|e| PebbleError::Storage(format!("Invalid conditions JSON: {e}")))?;
    evaluate_doc(&doc, msg)
}

pub(crate) fn evaluate_doc(doc: &ConditionsDoc, msg: &Message) -> Result<bool> {
    if doc.operator != "and" {
        return Err(PebbleError::Storage(format!(
            "Unsupported conditions operator: {} (only 'and' is supported)",
            doc.operator
        )));
    }
    for c in &doc.conditions {
        if !evaluate_predicate(c.field, c.op, &c.value, msg)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn evaluate_predicate(
    field: ConditionField,
    op: ConditionOp,
    value: &str,
    msg: &Message,
) -> Result<bool> {
    use ConditionField::*;
    let v = value.to_lowercase();
    let haystack_lc;
    let haystack: &str = match field {
        From => {
            haystack_lc = format!("{} {}", msg.from_name, msg.from_address).to_lowercase();
            &haystack_lc
        }
        To => {
            return Ok(match_addresses(&msg.to_list, op, &v));
        }
        Subject => {
            haystack_lc = msg.subject.to_lowercase();
            &haystack_lc
        }
        Body => {
            haystack_lc = msg.body_text.to_lowercase();
            &haystack_lc
        }
        Domain => {
            let at = msg.from_address.find('@').unwrap_or(msg.from_address.len());
            haystack_lc = msg.from_address[at.saturating_add(1)..].to_lowercase();
            &haystack_lc
        }
        HasAttachment => {
            return evaluate_attachment_predicate(op, &v, msg.has_attachments);
        }
    };
    Ok(string_op(haystack, op, &v))
}

fn string_op(haystack: &str, op: ConditionOp, needle: &str) -> bool {
    use ConditionOp::*;
    match op {
        Contains => haystack.contains(needle),
        NotContains => !haystack.contains(needle),
        Equals => haystack == needle,
        Starts => haystack.starts_with(needle),
        Ends => haystack.ends_with(needle),
    }
}

fn match_addresses(addrs: &[EmailAddress], op: ConditionOp, needle: &str) -> bool {
    use ConditionOp::*;
    let any_contains = addrs
        .iter()
        .any(|a| a.address.to_lowercase().contains(needle));
    let any_equals = addrs.iter().any(|a| a.address.to_lowercase() == needle);
    let any_starts = addrs
        .iter()
        .any(|a| a.address.to_lowercase().starts_with(needle));
    let any_ends = addrs
        .iter()
        .any(|a| a.address.to_lowercase().ends_with(needle));
    match op {
        Contains => any_contains,
        NotContains => !any_contains,
        Equals => any_equals,
        Starts => any_starts,
        Ends => any_ends,
    }
}

fn evaluate_attachment_predicate(op: ConditionOp, value: &str, has: bool) -> Result<bool> {
    use ConditionOp::*;
    let expected = match value {
        "true" | "yes" | "1" => true,
        "false" | "no" | "0" => false,
        other => {
            return Err(PebbleError::Storage(format!(
                "has_attachment value '{}' not recognized (true/yes/1 or false/no/0)",
                other
            )));
        }
    };
    Ok(match op {
        Equals => has == expected,
        Contains | NotContains | Starts | Ends => {
            // ops other than equals don't make sense for booleans
            return Err(PebbleError::Storage(format!(
                "has_attachment only supports op='equals', got op='{}'",
                match op {
                    Contains => "contains",
                    NotContains => "not_contains",
                    Starts => "starts_with",
                    Ends => "ends_with",
                    Equals => unreachable!(),
                }
            )));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pebble_core::Message;

    fn msg() -> Message {
        Message {
            id: "m1".into(),
            account_id: "acc1".into(),
            remote_id: "u1".into(),
            message_id_header: Some("<m1@x>".into()),
            in_reply_to: None,
            references_header: None,
            thread_id: None,
            subject: "URGENT: deploy now".into(),
            snippet: String::new(),
            from_address: "Boss@Example.COM".into(),
            from_name: "Alice".into(),
            to_list: vec![
                EmailAddress {
                    name: None,
                    address: "me@x.com".into(),
                },
                EmailAddress {
                    name: None,
                    address: "Bob@Y.com".into(),
                },
            ],
            cc_list: vec![],
            bcc_list: vec![],
            body_text: "Hello team".into(),
            body_html_raw: String::new(),
            has_attachments: true,
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

    fn cond(op: &str, conds: &[(&str, &str, &str)]) -> String {
        let arr: Vec<serde_json::Value> = conds
            .iter()
            .map(|(f, o, v)| {
                serde_json::json!({
                    "field": f, "op": o, "value": v,
                })
            })
            .collect();
        serde_json::json!({ "operator": op, "conditions": arr }).to_string()
    }

    #[test]
    fn from_contains_case_insensitive() {
        let c = cond("and", &[("from", "contains", "BOSS@example.com")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn from_contains_name() {
        let c = cond("and", &[("from", "contains", "alice")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn to_contains_any() {
        let c = cond("and", &[("to", "contains", "bob@y.com")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn to_not_contains_all() {
        let c = cond("and", &[("to", "not_contains", "nobody@nowhere")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn subject_starts_with() {
        let c = cond("and", &[("subject", "starts_with", "urgent")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn body_equals() {
        let c = cond("and", &[("body", "equals", "hello team")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn domain_ends_with() {
        let c = cond("and", &[("domain", "ends_with", "com")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn has_attachment_true() {
        let c = cond("and", &[("has_attachment", "equals", "true")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn has_attachment_yes_alias() {
        let c = cond("and", &[("has_attachment", "equals", "yes")]);
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn and_combines_all() {
        let c = cond(
            "and",
            &[
                ("from", "contains", "boss"),
                ("subject", "contains", "urgent"),
            ],
        );
        assert!(evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn and_short_circuits_false() {
        let c = cond(
            "and",
            &[
                ("from", "contains", "boss"),
                ("subject", "contains", "nonexistent"),
            ],
        );
        assert!(!evaluate(&c, &msg()).unwrap());
    }
    #[test]
    fn or_operator_rejected() {
        let c = cond("or", &[("from", "contains", "boss")]);
        assert!(evaluate(&c, &msg()).is_err());
    }
    #[test]
    fn bad_has_attachment_value_rejected() {
        let c = cond("and", &[("has_attachment", "equals", "maybe")]);
        assert!(evaluate(&c, &msg()).is_err());
    }
}
