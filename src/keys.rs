//! Key dispatch. Sidebar plugins receive PTY bytes; cmux owns the prefix
//! chord. `Esc` never quits. `Ctrl-C` and `q` exit cleanly.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Overlay};

/// Help rows shown on `?`.
#[must_use]
pub fn help_lines() -> Vec<(&'static str, &'static str)> {
    vec![
        ("K / Tab", "cycle view (List / Table / Kanban)"),
        ("Shift+Tab", "cycle view backwards"),
        ("j k ↑ ↓", "move selection"),
        ("h l ← →", "kanban: column (or retag in move mode)"),
        ("gg / G", "first / last"),
        ("v", "move mode: h/l writes bd update -s"),
        ("Enter", "detail modal (bd show)"),
        ("d", "toggle inline detail pane"),
        ("c", "claim (bd update --claim)"),
        ("x", "close (bd close -r, prompts)"),
        ("s", "set status, then 1-9"),
        ("p", "set priority, then 0-4"),
        ("a", "create bead"),
        ("e", "edit selected bead"),
        ("n", "add note (bd note)"),
        ("m", "add comment (bd comment)"),
        ("A", "assign to a live cmux pane"),
        ("/", "filter. Esc clears"),
        ("R", "toggle ready (bd ready --json)"),
        ("C", "show or hide closed"),
        ("S", "scope: repo ↔ global"),
        ("o", "table: cycle sort"),
        ("r", "refresh from bd"),
        ("?", "this help"),
        ("q / Ctrl+C", "quit"),
        ("Esc", "back out a layer (never quits)"),
    ]
}

/// Handle one key. Returns `true` when the event loop should exit.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return true;
    }

    if matches!(
        app.overlay,
        Overlay::Filter | Overlay::CloseReason | Overlay::Note | Overlay::Comment | Overlay::Create
    ) {
        return handle_prompt(app, key);
    }

    if app.overlay == Overlay::Assign {
        return handle_assign(app, key);
    }

    if app.overlay == Overlay::Help {
        app.escape();
        return false;
    }

    if app.overlay == Overlay::StatusPick {
        match key.code {
            KeyCode::Char(digit @ '1'..='9') => {
                app.pick_status((digit as u8 - b'1') as usize);
            }
            KeyCode::Esc => app.escape(),
            _ => app.escape(),
        }
        return false;
    }

    if app.overlay == Overlay::PriorityPick {
        match key.code {
            KeyCode::Char(digit @ '0'..='4') => app.pick_priority(digit as u8 - b'0'),
            KeyCode::Esc => app.escape(),
            _ => app.escape(),
        }
        return false;
    }

    if app.overlay == Overlay::Detail {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.escape(),
            _ => app.escape(),
        }
        return false;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        app.g_pending = false;
        match key.code {
            KeyCode::Char('d') => app.nav_vert(8),
            KeyCode::Char('u') => app.nav_vert(-8),
            KeyCode::Char('f') => app.nav_vert(16),
            KeyCode::Char('b') => app.nav_vert(-16),
            _ => {}
        }
        return false;
    }

    if app.g_pending {
        app.g_pending = false;
        if key.code == KeyCode::Char('g') {
            app.nav_vert(i32::MIN / 2);
        }
        return false;
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            return true;
        }
        KeyCode::Esc => app.escape(),
        KeyCode::Char('?') => app.open_help(),
        KeyCode::Char('K') | KeyCode::Tab => app.cycle_view(false),
        KeyCode::BackTab => app.cycle_view(true),
        KeyCode::Char('j') | KeyCode::Down => app.nav_vert(1),
        KeyCode::Char('k') | KeyCode::Up => app.nav_vert(-1),
        KeyCode::Char('h') | KeyCode::Left => app.nav_horiz(-1),
        KeyCode::Char('l') | KeyCode::Right => app.nav_horiz(1),
        KeyCode::Char('G') | KeyCode::End => app.nav_vert(i32::MAX / 2),
        KeyCode::Home => app.nav_vert(i32::MIN / 2),
        KeyCode::Char('g') => app.g_pending = true,
        KeyCode::Enter => app.open_detail(),
        KeyCode::Char('d') => app.toggle_detail_pane(),
        KeyCode::Char('c') => app.claim_selected(),
        KeyCode::Char('x') => app.open_close_reason(),
        KeyCode::Char('s') => app.open_status_pick(),
        KeyCode::Char('p') => app.open_priority_pick(),
        KeyCode::Char('a') => app.open_create(),
        KeyCode::Char('e') => app.open_edit(),
        KeyCode::Char('n') => app.open_note(),
        KeyCode::Char('m') => app.open_comment(),
        KeyCode::Char('A') => app.open_assign(),
        KeyCode::Char('v') => app.toggle_move_mode(),
        KeyCode::Char('/') => app.open_filter(),
        KeyCode::Char('R') => app.toggle_ready(),
        KeyCode::Char('C') => app.toggle_closed(),
        KeyCode::Char('S') => app.toggle_scope(),
        KeyCode::Char('o') => app.cycle_sort(),
        KeyCode::Char('r') => {
            app.status_msg.clear();
            app.reload();
        }
        _ => {}
    }

    false
}

fn handle_assign(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc => app.escape(),
        KeyCode::Enter => app.confirm_assign(),
        KeyCode::Char('j') | KeyCode::Down => app.nav_vert(1),
        KeyCode::Char('k') | KeyCode::Up => app.nav_vert(-1),
        _ => {}
    }
    false
}

fn handle_prompt(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc => app.escape(),
        KeyCode::Enter => {
            if app.overlay == Overlay::Create {
                app.submit_form();
            } else {
                app.submit_prompt();
            }
        }
        KeyCode::Backspace => app.prompt_backspace(),
        KeyCode::Tab | KeyCode::Down if app.overlay == Overlay::Create => app.form.next_field(),
        KeyCode::BackTab | KeyCode::Up if app.overlay == Overlay::Create => app.form.prev_field(),
        KeyCode::Left if app.overlay == Overlay::Create => app.form.nudge(-1),
        KeyCode::Right if app.overlay == Overlay::Create => app.form.nudge(1),
        KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            app.prompt_push(ch);
        }
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::BoardView;
    use std::path::PathBuf;

    fn press(app: &mut App, code: KeyCode) -> bool {
        handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl(app: &mut App, ch: char) -> bool {
        handle_key(app, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL))
    }

    #[test]
    fn ctrl_c_quits_instead_of_claiming() {
        let mut app = App::new(Some(PathBuf::from("/tmp")));
        app.selected = Some("demo-1".into());
        assert!(ctrl(&mut app, 'c'));
        assert!(app.should_quit);
    }

    #[test]
    fn plain_c_does_not_quit() {
        let mut app = App::new(Some(PathBuf::from("/tmp")));
        assert!(!press(&mut app, KeyCode::Char('c')));
        assert!(!app.should_quit);
    }

    #[test]
    fn esc_never_quits() {
        let mut app = App::new(Some(PathBuf::from("/tmp")));
        app.filter = "x".into();
        assert!(!press(&mut app, KeyCode::Esc));
        assert!(!app.should_quit);
        assert!(app.filter.is_empty());
    }

    #[test]
    fn g_then_x_does_not_open_close() {
        let mut app = App::new(Some(PathBuf::from("/tmp")));
        press(&mut app, KeyCode::Char('g'));
        assert!(app.g_pending);
        press(&mut app, KeyCode::Char('x'));
        assert!(!app.g_pending);
        assert_eq!(app.overlay, Overlay::None);
    }

    #[test]
    fn tab_cycles_views() {
        let mut app = App::new(Some(PathBuf::from("/tmp")));
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.view, BoardView::Table);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.view, BoardView::Kanban);
    }
}
