//! Serde types mirroring `bd ... --json` (beads v0.60+).
//!
//! `bd list --json` and `bd ready --json` return a flat array of issues.
//! `bd show --json` returns a one-element array whose `dependencies`
//! entries may be full issue objects. Notes/comments/labels accept several
//! shapes so a newer `bd` cannot break the board.

use serde::Deserialize;
use serde_json::Value;

/// A dependency edge or an expanded issue from `bd show`.
#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct Dependency {
    #[serde(default)]
    pub issue_id: Option<String>,
    #[serde(default)]
    pub depends_on_id: Option<String>,
    #[serde(rename = "type", default)]
    pub dep_type: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

impl Dependency {
    /// The id of the other bead this edge points at, whichever shape we got.
    #[must_use]
    pub fn other_id(&self) -> Option<&str> {
        self.depends_on_id.as_deref().or(self.id.as_deref())
    }

    /// One-line label for the detail pane.
    #[must_use]
    pub fn label(&self) -> String {
        match (
            self.other_id(),
            self.title.as_deref(),
            self.dep_type.as_deref(),
        ) {
            (Some(id), Some(title), Some(kind)) => format!("{kind}: {id} - {title}"),
            (Some(id), Some(title), None) => format!("{id} - {title}"),
            (Some(id), None, Some(kind)) => format!("{kind}: {id}"),
            (Some(id), None, None) => id.to_string(),
            _ => "(dependency)".to_string(),
        }
    }
}

/// One `bd` issue. This is the board's only record type; the plugin never
/// invents a store of its own.
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct Bead {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub priority: u8,
    #[serde(rename = "issue_type", default)]
    pub issue_type: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default, alias = "parent_id")]
    pub parent: Option<String>,
    #[serde(default)]
    pub labels: Value,
    #[serde(default)]
    pub notes: Value,
    #[serde(default)]
    pub comments: Value,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub dependency_count: u32,
    #[serde(default)]
    pub dependent_count: u32,
    #[serde(default)]
    pub comment_count: u32,
}

fn default_status() -> String {
    "open".to_string()
}

impl Bead {
    /// Raw assignee / owner string as `bd` stored it.
    #[must_use]
    pub fn assignee_raw(&self) -> Option<&str> {
        self.assignee
            .as_deref()
            .filter(|value| !value.is_empty())
            .or_else(|| self.owner.as_deref().filter(|value| !value.is_empty()))
    }

    /// Display assignee. `cmux:{workspace}/{pane}` is kept intact; emails
    /// show the local-part.
    #[must_use]
    pub fn assignee_display(&self) -> &str {
        match self.assignee_raw() {
            Some(value) if value.starts_with("cmux:") => value,
            Some(value) => value.split('@').next().unwrap_or(value),
            None => "-",
        }
    }

    /// Labels as a flat string list.
    #[must_use]
    pub fn label_list(&self) -> Vec<String> {
        extract_texts(&self.labels)
    }

    /// Notes from `bd show` (string, list, or objects with text/body/note).
    #[must_use]
    pub fn note_lines(&self) -> Vec<String> {
        extract_texts(&self.notes)
    }

    /// Comments from `bd show`.
    #[must_use]
    pub fn comment_lines(&self) -> Vec<String> {
        extract_texts(&self.comments)
    }

    /// Whether `bd` reports this issue as closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.status == "closed"
    }

    /// Whether this issue is an epic (parent candidates).
    #[must_use]
    pub fn is_epic(&self) -> bool {
        self.issue_type == "epic"
    }

    /// Single-line search haystack used by the `/` filter.
    #[must_use]
    pub fn haystack(&self) -> String {
        format!(
            "{} {} {} {} {} {} {}",
            self.id,
            self.title,
            self.description,
            self.issue_type,
            self.status,
            self.assignee_display(),
            self.label_list().join(" ")
        )
        .to_lowercase()
    }
}

/// Pull human text out of a `bd --json` notes/labels/comments value.
#[must_use]
pub fn extract_texts(value: &Value) -> Vec<String> {
    match value {
        Value::Null => Vec::new(),
        Value::String(text) if !text.is_empty() => vec![text.clone()],
        Value::Array(items) => items.iter().flat_map(extract_texts).collect(),
        Value::Object(map) => {
            for key in ["text", "content", "body", "note", "comment", "name"] {
                if let Some(Value::String(text)) = map.get(key)
                    && !text.is_empty()
                {
                    return vec![text.clone()];
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_texts_accepts_string_array_and_objects() {
        assert!(extract_texts(&Value::Null).is_empty());
        assert_eq!(extract_texts(&serde_json::json!("ship it")), ["ship it"]);
        assert_eq!(
            extract_texts(&serde_json::json!([{"text": "n1"}, {"body": "n2"}])),
            ["n1", "n2"]
        );
        assert_eq!(extract_texts(&serde_json::json!(["a", "b"])), ["a", "b"]);
    }
}
