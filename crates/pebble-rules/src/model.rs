//! Rule condition/action model. Byte-compatible with the frontend
//! schema in `frontend/src/features/settings/rule-json.ts`.
//!
//! conditions JSON shape (per `serializeRuleConditions`):
//!   `{"operator":"and","conditions":[{field,op,value}, ...]}`
//!
//! actions JSON shape (per `serializeRuleActions`):
//!   `[{"type":"AddLabel","value":"工作"}, ...]`
//!   (`MarkRead` and `Archive` carry no `value`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionField {
    From,
    To,
    Subject,
    Body,
    HasAttachment,
    Domain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConditionOp {
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "not_contains")]
    NotContains,
    #[serde(rename = "equals")]
    Equals,
    #[serde(rename = "starts_with")]
    Starts,
    #[serde(rename = "ends_with")]
    Ends,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ActionType {
    AddLabel,
    MoveToFolder,
    MarkRead,
    Archive,
    SetKanbanColumn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    pub field: ConditionField,
    pub op: ConditionOp,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAction {
    #[serde(rename = "type")]
    pub action_type: ActionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Inner shape of the conditions JSON (`{"operator":"and","conditions":[...]}`).
#[derive(Debug, Deserialize)]
pub struct ConditionsDoc {
    pub operator: String,
    pub conditions: Vec<RuleCondition>,
}

/// Parse conditions JSON. Engine fails loudly so users see broken rules.
/// Returns Err on bad JSON or unsupported operator (the latter is checked
/// at evaluation time, since this helper only parses the shape).
pub fn parse_conditions(json: &str) -> Result<ConditionsDoc, serde_json::Error> {
    serde_json::from_str::<ConditionsDoc>(json)
}

/// Parse actions JSON (a JSON array).
pub fn parse_actions(json: &str) -> Result<Vec<RuleAction>, serde_json::Error> {
    serde_json::from_str::<Vec<RuleAction>>(json)
}
