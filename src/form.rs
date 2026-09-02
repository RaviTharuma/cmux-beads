//! Multi-field create / edit form. Fields match `bd create` / `bd update`.

use crate::bd::ISSUE_TYPES;

pub const F_TYPE: u8 = 0;
pub const F_PRIORITY: u8 = 1;
pub const F_TITLE: u8 = 2;
pub const F_DESC: u8 = 3;
pub const F_ASSIGNEE: u8 = 4;
pub const F_EPIC: u8 = 5;
pub const F_LABELS: u8 = 6;
pub const F_BACKLOG: u8 = 7;
pub const FIELDS: u8 = 8;

/// In-progress `bd create` / `bd update` form.
#[derive(Debug, Clone)]
pub struct BeadForm {
    pub title: String,
    pub description: String,
    pub assignee: String,
    pub labels: String,
    pub type_index: usize,
    pub priority: u8,
    /// 0 = no epic; otherwise index + 1 into `epics`.
    pub epic_idx: usize,
    pub deferred: bool,
    pub field: u8,
    pub epics: Vec<(String, String)>,
    /// `Some(id)` when editing an existing bead.
    pub edit_id: Option<String>,
}

impl BeadForm {
    /// Empty create form.
    #[must_use]
    pub fn new(epics: Vec<(String, String)>) -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            assignee: String::new(),
            labels: String::new(),
            type_index: 0,
            priority: 2,
            epic_idx: 0,
            deferred: false,
            field: F_TITLE,
            epics,
            edit_id: None,
        }
    }

    /// Current type name from [`ISSUE_TYPES`].
    #[must_use]
    pub fn issue_type(&self) -> &'static str {
        ISSUE_TYPES[self.type_index.min(ISSUE_TYPES.len().saturating_sub(1))]
    }

    /// Selected parent epic id, or empty.
    #[must_use]
    pub fn parent_id(&self) -> &str {
        if self.epic_idx == 0 {
            ""
        } else {
            self.epics
                .get(self.epic_idx - 1)
                .map(|(id, _)| id.as_str())
                .unwrap_or("")
        }
    }

    /// One-line epic picker label.
    #[must_use]
    pub fn epic_label(&self) -> String {
        if self.epic_idx == 0 {
            return "No epic".to_string();
        }
        match self.epics.get(self.epic_idx - 1) {
            Some((id, title)) => format!("{id} - {}", title.chars().take(28).collect::<String>()),
            None => "No epic".to_string(),
        }
    }

    pub fn next_field(&mut self) {
        self.field = (self.field + 1) % FIELDS;
    }

    pub fn prev_field(&mut self) {
        self.field = (self.field + FIELDS - 1) % FIELDS;
    }

    pub fn nudge(&mut self, delta: i32) {
        match self.field {
            F_TYPE => {
                let len = ISSUE_TYPES.len() as i32;
                self.type_index = (self.type_index as i32 + delta).rem_euclid(len) as usize;
            }
            F_PRIORITY => {
                self.priority = (self.priority as i32 + delta).clamp(0, 4) as u8;
            }
            F_EPIC => {
                let len = (self.epics.len() + 1) as i32;
                self.epic_idx = (self.epic_idx as i32 + delta).rem_euclid(len) as usize;
            }
            _ => {}
        }
    }

    pub fn input_char(&mut self, ch: char) {
        match self.field {
            F_TITLE => self.title.push(ch),
            F_DESC => self.description.push(ch),
            F_ASSIGNEE => self.assignee.push(ch),
            F_LABELS => self.labels.push(ch),
            F_PRIORITY => {
                if let Some(digit) = ch.to_digit(10)
                    && digit <= 4
                {
                    self.priority = digit as u8;
                }
            }
            F_BACKLOG if ch == ' ' => self.deferred = !self.deferred,
            _ => {}
        }
    }

    pub fn backspace(&mut self) {
        let target = match self.field {
            F_TITLE => &mut self.title,
            F_DESC => &mut self.description,
            F_ASSIGNEE => &mut self.assignee,
            F_LABELS => &mut self.labels,
            _ => return,
        };
        target.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_and_nudge_wraps_type() {
        let mut form = BeadForm::new(vec![("e-1".into(), "Epic".into())]);
        assert_eq!(form.field, F_TITLE);
        form.next_field();
        assert_eq!(form.field, F_DESC);
        form.field = F_TYPE;
        form.nudge(-1);
        assert_eq!(form.issue_type(), *ISSUE_TYPES.last().unwrap());
        form.field = F_EPIC;
        form.nudge(1);
        assert_eq!(form.parent_id(), "e-1");
    }
}
