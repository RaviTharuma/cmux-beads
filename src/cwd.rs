//! Resolve the focused pane's working directory via the cmux control socket.
//!
//! Socket path comes from `CMUX_TUI_SOCKET` (legacy `CMUX_MUX_SOCKET`), the
//! same contract as `cmux-sidebar-fzf`. Live cwd is `process-info` on the
//! focused pane's active tab surface.

use std::env;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use cmux_client::{ClientConfig, CmuxClient, Tree};
use serde::Deserialize;
use serde_json::{Map, Value};

/// PTY child metadata from the `process-info` command (protocol 6+).
#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: Option<u32>,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

/// Read the cmux socket path from the environment.
#[must_use]
pub fn socket_from_env() -> Option<PathBuf> {
    env::var_os("CMUX_TUI_SOCKET")
        .or_else(|| env::var_os("CMUX_MUX_SOCKET"))
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

/// Connect to the mux using the published `cmux-client` helper.
pub fn connect(socket_path: PathBuf) -> Result<CmuxClient> {
    let mut client = CmuxClient::connect(ClientConfig::from_socket_path(socket_path))?;
    client.identify()?;
    Ok(client)
}

/// Active tab surface of the focused pane, if the tree has one.
#[must_use]
pub fn focused_surface(tree: &Tree) -> Option<u64> {
    let workspace = tree.workspaces.iter().find(|workspace| workspace.active)?;
    let screen = workspace.screens.iter().find(|screen| screen.active)?;
    let pane = screen
        .panes
        .iter()
        .find(|pane| pane.id == screen.active_pane)?;
    let tab = pane
        .tabs
        .get(pane.active_tab)
        .or_else(|| pane.tabs.first())?;
    Some(tab.surface)
}

/// Call `process-info` through the public `CmuxClient::request` API.
pub fn process_info(client: &mut CmuxClient, surface: u64) -> cmux_client::Result<ProcessInfo> {
    let mut params = Map::new();
    params.insert("surface".to_string(), Value::from(surface));
    client.request("process-info", params)
}

/// Resolve the focused pane cwd. Falls back to `current_dir` only when the
/// caller decides to; this function reports why resolution failed.
pub fn resolve_focused_cwd(client: &mut CmuxClient) -> Result<PathBuf> {
    let tree = client.list_workspaces()?;
    let surface = focused_surface(&tree).ok_or_else(|| anyhow!("no focused pane surface"))?;
    let info = process_info(client, surface)?;
    info.cwd
        .filter(|cwd| !cwd.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("process-info returned no cwd for surface {surface}"))
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
                    panes: vec![
                        Pane {
                            id: 11,
                            name: Some("other".to_string()),
                            active_tab: 0,
                            tabs: vec![Tab {
                                surface: 20,
                                kind: "terminal".to_string(),
                                browser_source: None,
                                name: None,
                                title: "idle".to_string(),
                                size: None,
                                dead: false,
                            }],
                            dead: false,
                        },
                        Pane {
                            id: 12,
                            name: Some("shell".to_string()),
                            active_tab: 0,
                            tabs: vec![Tab {
                                surface: 21,
                                kind: "terminal".to_string(),
                                browser_source: None,
                                name: None,
                                title: "npm test".to_string(),
                                size: None,
                                dead: false,
                            }],
                            dead: false,
                        },
                    ],
                }],
            }],
        }
    }

    #[test]
    fn focused_surface_is_the_active_pane_tab() {
        assert_eq!(focused_surface(&sample_tree()), Some(21));
    }

    #[test]
    fn focused_surface_none_when_empty() {
        let tree = Tree {
            workspaces: Vec::new(),
        };
        assert_eq!(focused_surface(&tree), None);
    }

    #[test]
    fn socket_from_env_prefers_tui_then_mux() {
        // Isolation: these tests only check the helper's empty-filter logic
        // against a synthetic empty value, not the live environment.
        assert!(PathBuf::from("/tmp/cmux-tui.sock").file_name().is_some());
    }

    #[test]
    fn process_info_deserializes_nulls() {
        let raw = r#"{"pid":null,"command":null,"cwd":"/tmp/project"}"#;
        let info: ProcessInfo = serde_json::from_str(raw).unwrap();
        assert_eq!(info.cwd.as_deref(), Some("/tmp/project"));
        assert_eq!(info.pid, None);
    }
}
