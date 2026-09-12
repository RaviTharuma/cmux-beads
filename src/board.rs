//! Pure board presentation: filter, group, views, and selection helpers.

use crate::bd::{Bead, KNOWN_STATUSES};
use crate::sessions;

/// In-process view. Switching does not respawn or reload `bd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardView {
    List,
    Table,
    Kanban,
}

impl BoardView {
    /// Cycle List → Table → Kanban → List.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::List => Self::Table,
            Self::Table => Self::Kanban,
            Self::Kanban => Self::List,
        }
    }

    /// Cycle backwards.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::List => Self::Kanban,
            Self::Table => Self::List,
            Self::Kanban => Self::Table,
        }
    }

    /// Header label.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::List => "List",
            Self::Table => "Table",
            Self::Kanban => "Kanban",
        }
    }
}

/// Focus filter for the board. Does not change the `bd` store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusScope {
    /// Every bead on the loaded board.
    #[default]
    Host,
    /// Beads assigned to the live focused pane (`cmux:{ws}/{pane}`).
    Focus,
    /// Beads with any cmux pane assignee.
    Assigned,
}

impl FocusScope {
    /// Cycle Host → Focus → Assigned → Host.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Host => Self::Focus,
            Self::Focus => Self::Assigned,
            Self::Assigned => Self::Host,
        }
    }

    /// Header label.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Focus => "focus",
            Self::Assigned => "assigned",
        }
    }
}

/// Whether `bead` matches the active focus scope.
///
/// When `Focus` is selected but no live pane is active, every bead matches so
/// the board does not go blank while cmux reconnects.
#[must_use]
pub fn matches_focus_scope(bead: &Bead, scope: FocusScope, focused_assignee: Option<&str>) -> bool {
    match scope {
        FocusScope::Host => true,
        FocusScope::Focus => match focused_assignee {
            Some(want) => bead.assignee_raw() == Some(want),
            None => true,
        },
        FocusScope::Assigned => bead
            .assignee_raw()
            .is_some_and(|raw| sessions::parse_assignee(raw).is_some()),
    }
}

/// Table sort key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Status,
    Priority,
    Changed,
}

impl SortKey {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Status => Self::Priority,
            Self::Priority => Self::Changed,
            Self::Changed => Self::Status,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Priority => "priority",
            Self::Changed => "changed",
        }
    }
}

/// Whether `bead` matches a `/` query. Empty query matches everything.
#[must_use]
pub fn matches_filter(bead: &Bead, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    bead.haystack().contains(&query.to_lowercase())
}

/// Stable status sort key: known statuses first, then custom names.
#[must_use]
pub fn status_rank(status: &str) -> (u8, String) {
    if let Some(index) = KNOWN_STATUSES.iter().position(|known| *known == status) {
        return (index as u8, String::new());
    }
    (KNOWN_STATUSES.len() as u8, status.to_string())
}

/// Visible beads: filtered, then ordered by the active sort.
#[must_use]
pub fn visible_beads<'a>(beads: &'a [Bead], query: &str, sort: SortKey) -> Vec<&'a Bead> {
    let mut rows: Vec<&Bead> = beads
        .iter()
        .filter(|bead| matches_filter(bead, query))
        .collect();
    rows.sort_by(|left, right| match sort {
        SortKey::Status => status_rank(&left.status)
            .cmp(&status_rank(&right.status))
            .then(left.priority.cmp(&right.priority))
            .then(left.id.cmp(&right.id)),
        SortKey::Priority => left
            .priority
            .cmp(&right.priority)
            .then(left.id.cmp(&right.id)),
        SortKey::Changed => right
            .updated_at
            .cmp(&left.updated_at)
            .then(left.id.cmp(&right.id)),
    });
    rows
}

/// Index of `id` in `rows`, if present.
#[must_use]
pub fn index_of(rows: &[&Bead], id: &str) -> Option<usize> {
    rows.iter().position(|bead| bead.id == id)
}

/// Statuses present on the board plus the known set, de-duplicated, in rank order.
#[must_use]
pub fn status_choices(beads: &[Bead]) -> Vec<String> {
    let mut choices: Vec<String> = KNOWN_STATUSES
        .iter()
        .map(|status| (*status).to_string())
        .collect();
    for bead in beads {
        if !choices.iter().any(|status| status == &bead.status) && !bead.status.is_empty() {
            choices.push(bead.status.clone());
        }
    }
    choices
}

/// Kanban columns with a focus scope filter.
#[must_use]
pub fn kanban_columns<'a>(
    beads: &'a [Bead],
    query: &str,
    include_closed: bool,
    focus: FocusScope,
    focused_assignee: Option<&str>,
) -> Vec<(String, Vec<&'a Bead>)> {
    let choices: Vec<String> = status_choices(beads)
        .into_iter()
        .filter(|status| include_closed || status != "closed")
        .collect();
    choices
        .into_iter()
        .map(|status| {
            let mut cards: Vec<&Bead> = beads
                .iter()
                .filter(|bead| {
                    bead.status == status
                        && matches_filter(bead, query)
                        && matches_focus_scope(bead, focus, focused_assignee)
                        && (include_closed || !bead.is_closed() || status == "closed")
                })
                .collect();
            cards.sort_by(|left, right| {
                left.priority
                    .cmp(&right.priority)
                    .then(left.id.cmp(&right.id))
            });
            (status, cards)
        })
        .collect()
}

/// Adjacent status in kanban order.
#[must_use]
pub fn adjacent_status(
    columns: &[(String, Vec<&Bead>)],
    current: &str,
    delta: i32,
) -> Option<String> {
    let index = columns.iter().position(|(status, _)| status == current)?;
    let next = index.saturating_add_signed(delta as isize);
    columns
        .get(next.min(columns.len() - 1))
        .map(|(status, _)| status.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bd::parse_list;

    fn fixture() -> Vec<Bead> {
        parse_list(include_str!("../tests/fixtures/list.json")).unwrap()
    }

    #[test]
    fn empty_filter_keeps_all_in_status_order() {
        let beads = fixture();
        let rows = visible_beads(&beads, "", SortKey::Status);
        assert_eq!(
            rows.iter().map(|bead| bead.id.as_str()).collect::<Vec<_>>(),
            ["demo-1", "demo-2", "demo-3"]
        );
    }

    #[test]
    fn filter_is_case_insensitive_over_id_and_title() {
        let beads = fixture();
        let rows = visible_beads(&beads, "DARK", SortKey::Status);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "demo-1");
        assert!(visible_beads(&beads, "demo-3", SortKey::Status).len() == 1);
        assert!(visible_beads(&beads, "no-such-bead", SortKey::Status).is_empty());
    }

    #[test]
    fn index_of_tracks_id_across_filter() {
        let beads = fixture();
        let rows = visible_beads(&beads, "", SortKey::Status);
        assert_eq!(index_of(&rows, "demo-2"), Some(1));
        assert_eq!(index_of(&rows, "missing"), None);
    }

    #[test]
    fn status_choices_include_known_and_custom() {
        let mut beads = fixture();
        beads.push(Bead {
            id: "x".into(),
            status: "triage".into(),
            ..Bead::default()
        });
        let choices = status_choices(&beads);
        assert!(choices.contains(&"open".to_string()));
        assert!(choices.contains(&"triage".to_string()));
        assert_eq!(choices[0], "open");
    }

    #[test]
    fn kanban_columns_are_statuses() {
        let beads = fixture();
        let columns = kanban_columns(&beads, "", false, FocusScope::Host, None);
        let open = columns.iter().find(|(status, _)| status == "open").unwrap();
        assert_eq!(open.1[0].id, "demo-1");
        let next = adjacent_status(&columns, "open", 1).unwrap();
        assert_eq!(next, "in_progress");
    }

    #[test]
    fn view_cycle_is_closed() {
        assert_eq!(BoardView::List.next(), BoardView::Table);
        assert_eq!(BoardView::Table.next(), BoardView::Kanban);
        assert_eq!(BoardView::Kanban.next(), BoardView::List);
        assert_eq!(BoardView::List.prev(), BoardView::Kanban);
    }

    #[test]
    fn focus_scope_filters_assigned_pane() {
        let mut beads = fixture();
        beads[0].assignee = Some("cmux:1/12".into());
        beads[1].assignee = Some("cmux:1/99".into());
        assert!(matches_focus_scope(
            &beads[0],
            FocusScope::Focus,
            Some("cmux:1/12")
        ));
        assert!(!matches_focus_scope(
            &beads[1],
            FocusScope::Focus,
            Some("cmux:1/12")
        ));
        assert!(matches_focus_scope(
            &beads[2],
            FocusScope::Host,
            Some("cmux:1/12")
        ));
        assert!(matches_focus_scope(&beads[0], FocusScope::Assigned, None));
        assert!(!matches_focus_scope(&beads[2], FocusScope::Assigned, None));
        let columns = kanban_columns(&beads, "", false, FocusScope::Focus, Some("cmux:1/12"));
        let open = columns.iter().find(|(status, _)| status == "open").unwrap();
        assert_eq!(open.1.len(), 1);
        assert_eq!(open.1[0].id, "demo-1");
    }
}
