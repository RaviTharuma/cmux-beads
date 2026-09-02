//! Live cmux workspaces / screens / panes, and the assignee mapping.
//!
//! `bd` has no first-class chat or pane id. Assignment is stored on the
//! issue itself:
//!
//! - **assignee** = `cmux:{workspace_id}/{pane_id}`
//!
//! Display names come from the live tree at render time. No second database.

use cmux_client::{Pane, Tree};

/// Prefix written into the `bd` assignee field.
pub const ASSIGNEE_PREFIX: &str = "cmux:";

/// One live workspace from `list-workspaces` on `CMUX_TUI_SOCKET`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveWorkspace {
    pub id: u64,
    pub name: String,
    pub active: bool,
    pub pane_count: usize,
}

/// One live pane the user can assign a bead to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivePane {
    pub workspace_id: u64,
    pub workspace_name: String,
    pub screen_id: u64,
    pub screen_name: String,
    pub pane_id: u64,
    pub pane_title: String,
    pub active: bool,
}

impl LivePane {
    /// Stable `bd` assignee string for this pane.
    #[must_use]
    pub fn assignee(&self) -> String {
        encode_assignee(self.workspace_id, self.pane_id)
    }

    /// Human breadcrumb, fzf-plugin style.
    #[must_use]
    pub fn label(&self) -> String {
        format!(
            "{} > {} > {}",
            self.workspace_name, self.screen_name, self.pane_title
        )
    }
}

/// Encode a live pane as a `bd` assignee.
#[must_use]
pub fn encode_assignee(workspace_id: u64, pane_id: u64) -> String {
    format!("{ASSIGNEE_PREFIX}{workspace_id}/{pane_id}")
}

/// Parse `cmux:{workspace_id}/{pane_id}`.
#[must_use]
pub fn parse_assignee(raw: &str) -> Option<(u64, u64)> {
    let rest = raw.strip_prefix(ASSIGNEE_PREFIX)?;
    let (workspace, pane) = rest.split_once('/')?;
    Some((workspace.parse().ok()?, pane.parse().ok()?))
}

/// Flatten the cmux session tree into workspaces. Names and ids come from
/// the live socket; this never invents a workspace that is not in `tree`.
#[must_use]
pub fn flatten_live_workspaces(tree: &Tree) -> Vec<LiveWorkspace> {
    tree.workspaces
        .iter()
        .map(|workspace| LiveWorkspace {
            id: workspace.id,
            name: workspace.name.clone(),
            active: workspace.active,
            pane_count: workspace
                .screens
                .iter()
                .map(|screen| screen.panes.len())
                .sum(),
        })
        .collect()
}

/// Flatten the cmux session tree into assignable panes.
#[must_use]
pub fn flatten_live_panes(tree: &Tree) -> Vec<LivePane> {
    let mut panes = Vec::new();
    for workspace in &tree.workspaces {
        for screen in &workspace.screens {
            let screen_name = screen
                .name
                .clone()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("screen {}", screen.id));
            for pane in &screen.panes {
                panes.push(LivePane {
                    workspace_id: workspace.id,
                    workspace_name: workspace.name.clone(),
                    screen_id: screen.id,
                    screen_name: screen_name.clone(),
                    pane_id: pane.id,
                    pane_title: pane_title(pane),
                    active: workspace.active && screen.active && pane.id == screen.active_pane,
                });
            }
        }
    }
    panes
}

fn pane_title(pane: &Pane) -> String {
    if let Some(name) = pane.name.as_ref().filter(|name| !name.is_empty()) {
        return name.clone();
    }
    if let Some(tab) = pane.tabs.get(pane.active_tab).or_else(|| pane.tabs.first()) {
        if let Some(name) = tab.name.as_ref().filter(|name| !name.is_empty()) {
            return name.clone();
        }
        if !tab.title.is_empty() {
            return tab.title.clone();
        }
        return format!("{} tab", tab.kind);
    }
    format!("pane {}", pane.id)
}

/// Resolve a stored assignee against the live tree for display.
#[must_use]
pub fn resolve_assignee<'a>(raw: &str, panes: &'a [LivePane]) -> Option<&'a LivePane> {
    let (workspace_id, pane_id) = parse_assignee(raw)?;
    panes
        .iter()
        .find(|pane| pane.workspace_id == workspace_id && pane.pane_id == pane_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmux_client::{Layout, Pane, Screen, Tab, Workspace};

    fn sample_tree() -> Tree {
        Tree {
            workspaces: vec![Workspace {
                id: 1,
                name: "alpha".to_string(),
                active: true,
                screens: vec![Screen {
                    id: 10,
                    name: Some("editor".to_string()),
                    active: true,
                    active_pane: 12,
                    layout: Layout::Leaf { pane: 12 },
                    panes: vec![Pane {
                        id: 12,
                        name: Some("agent".to_string()),
                        active_tab: 0,
                        tabs: vec![Tab {
                            surface: 21,
                            kind: "terminal".to_string(),
                            browser_source: None,
                            name: None,
                            title: "claude".to_string(),
                            size: None,
                            dead: false,
                        }],
                        dead: false,
                    }],
                }],
            }],
        }
    }

    #[test]
    fn encode_parse_roundtrip() {
        let encoded = encode_assignee(1, 12);
        assert_eq!(encoded, "cmux:1/12");
        assert_eq!(parse_assignee(&encoded), Some((1, 12)));
        assert_eq!(parse_assignee("user@example.com"), None);
    }

    #[test]
    fn flatten_and_resolve_live_pane() {
        let panes = flatten_live_panes(&sample_tree());
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].assignee(), "cmux:1/12");
        assert_eq!(panes[0].label(), "alpha > editor > agent");
        assert!(panes[0].active);
        let resolved = resolve_assignee("cmux:1/12", &panes).unwrap();
        assert_eq!(resolved.pane_id, 12);
    }

    #[test]
    fn flatten_live_workspaces_comes_from_the_socket_tree() {
        let workspaces = flatten_live_workspaces(&sample_tree());
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].id, 1);
        assert_eq!(workspaces[0].name, "alpha");
        assert!(workspaces[0].active);
        assert_eq!(workspaces[0].pane_count, 1);
        assert!(
            flatten_live_workspaces(&Tree {
                workspaces: Vec::new()
            })
            .is_empty()
        );
    }
}
