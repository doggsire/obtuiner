use std::{io::stdout, process::Command, time::Duration};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_kit::{
    handle_common_key, handle_modal_key, render_confirm_modal, render_layout, CommonAction,
    ConfirmModal, LayoutData, ModalChoice, SharedState,
};

#[derive(Clone)]
struct PowerAction {
    label: String,
    detail: Vec<String>,
    command: String,
}

enum AppMode {
    Browse,
    Confirm(PowerAction),
}

const STATUS_LINE: &str = "Powermenu | Type to jump | Enter: select | Esc: quit";

/// Index of the item that best matches `query`, or `None` if `query` is
/// empty or matches nothing. Exact matches win, then prefix matches, then
/// substring matches, then in-order fuzzy (subsequence) matches. Ties keep
/// the earliest item.
fn best_match_index(items: &[String], query: &str) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return None;
    }
    items
        .iter()
        .enumerate()
        .filter_map(|(i, label)| match_score(&label.to_lowercase(), &query).map(|score| (score, i)))
        .min_by_key(|&(score, _)| score)
        .map(|(_, i)| i)
}

/// Lower is better; `None` means no match at all.
fn match_score(label: &str, query: &str) -> Option<u8> {
    if label == query {
        Some(0)
    } else if label.starts_with(query) {
        Some(1)
    } else if label.contains(query) {
        Some(2)
    } else if is_subsequence(query, label) {
        Some(3)
    } else {
        None
    }
}

/// True if every character of `query` appears in `label`, in order (not
/// necessarily contiguous).
fn is_subsequence(query: &str, label: &str) -> bool {
    let mut label_chars = label.chars();
    query
        .chars()
        .all(|qc| label_chars.any(|lc| lc == qc))
}

/// True if the `hyprshutdown` graceful session-exit helper is on `PATH`.
fn hyprshutdown_available() -> bool {
    Command::new("which")
        .arg("hyprshutdown")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Build the fixed shutdown/reboot/sleep/logout menu. When `hyprshutdown` is
/// available, it's used for the session-affecting actions (logout, and as a
/// graceful wrapper for shutdown/reboot); otherwise direct systemd/loginctl
/// commands are used.
fn build_actions(use_hyprshutdown: bool) -> Vec<PowerAction> {
    let hint = if use_hyprshutdown {
        "hyprshutdown detected — used for graceful session exit.".to_string()
    } else {
        "hyprshutdown not found — using direct systemctl/loginctl commands.".to_string()
    };

    let shutdown_cmd = if use_hyprshutdown {
        "hyprshutdown -t 'Shutting down...' --post-cmd 'systemctl poweroff'".to_string()
    } else {
        "systemctl poweroff".to_string()
    };
    let reboot_cmd = if use_hyprshutdown {
        "hyprshutdown -t 'Restarting...' --post-cmd 'systemctl reboot'".to_string()
    } else {
        "systemctl reboot".to_string()
    };
    let logout_cmd = if use_hyprshutdown {
        "hyprshutdown".to_string()
    } else {
        "loginctl terminate-user \"$USER\"".to_string()
    };
    // Suspend doesn't need hyprshutdown's app-closing behavior; the session
    // resumes in place, so a direct command is used regardless.
    let sleep_cmd = "systemctl suspend".to_string();

    vec![
        PowerAction {
            label: "Shutdown".to_string(),
            detail: vec![hint.clone(), String::new(), format!("Command: {}", shutdown_cmd)],
            command: shutdown_cmd,
        },
        PowerAction {
            label: "Reboot".to_string(),
            detail: vec![hint.clone(), String::new(), format!("Command: {}", reboot_cmd)],
            command: reboot_cmd,
        },
        PowerAction {
            label: "Sleep".to_string(),
            detail: vec![String::new(), format!("Command: {}", sleep_cmd)],
            command: sleep_cmd,
        },
        PowerAction {
            label: "Logout".to_string(),
            detail: vec![hint, String::new(), format!("Command: {}", logout_cmd)],
            command: logout_cmd,
        },
    ]
}

pub fn run(_args: &[String]) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut state = SharedState::default();
    let actions = build_actions(hyprshutdown_available());
    let mut mode = AppMode::Browse;
    let mut status = STATUS_LINE.to_string();

    loop {
        if actions.is_empty() {
            state.selected = 0;
        } else {
            state.selected = state.selected.min(actions.len() - 1);
        }

        let items: Vec<String> = actions.iter().map(|a| a.label.clone()).collect();
        let details = actions
            .get(state.selected)
            .map(|a| a.detail.clone())
            .unwrap_or_else(|| vec!["No action selected.".to_string()]);

        terminal.draw(|frame| {
            let data = LayoutData {
                app_title: "Powermenu",
                left_title: "Actions",
                right_title: "Details",
                status_line: &status,
                items: &items,
                details: &details,
            };
            match &mode {
                AppMode::Confirm(action) => {
                    render_layout(frame, &state, &data);
                    render_confirm_modal(
                        frame,
                        &ConfirmModal {
                            title: format!(" Confirm {} ", action.label),
                            lines: vec![format!("Command: {}", action.command)],
                        },
                    );
                }
                AppMode::Browse => render_layout(frame, &state, &data),
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            if let Event::Key(key) = event {
                match &mode {
                    AppMode::Confirm(action) => {
                        let action = action.clone();
                        match handle_modal_key(key) {
                            ModalChoice::Confirmed => {
                                restore_terminal(&mut terminal)?;
                                println!("Running: {}", action.command);
                                let ok = Command::new("sh")
                                    .args(["-c", &action.command])
                                    .status()
                                    .map(|s| s.success())
                                    .unwrap_or(false);
                                if !ok {
                                    eprintln!("Command failed: {}", action.command);
                                }
                                return Ok(());
                            }
                            ModalChoice::Cancelled => {
                                mode = AppMode::Browse;
                                status = STATUS_LINE.to_string();
                            }
                            ModalChoice::Pending => {}
                        }
                    }
                    AppMode::Browse => {
                        let previous_query = state.query.clone();
                        let result = handle_common_key(&mut state, key, actions.len());
                        if state.query != previous_query {
                            if let Some(idx) = best_match_index(&items, &state.query) {
                                state.selected = idx;
                            }
                        }
                        match result {
                            CommonAction::Quit => break,
                            CommonAction::Activate => {
                                if let Some(action) = actions.get(state.selected) {
                                    mode = AppMode::Confirm(action.clone());
                                }
                            }
                            CommonAction::None => {}
                        }
                    }
                }
            }
        }
    }

    restore_terminal(&mut terminal)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, Clear(ClearType::All))?;
    let backend = CrosstermBackend::new(out);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        Clear(ClearType::All),
        MoveTo(0, 0),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_all_four_actions_in_order() {
        let actions = build_actions(true);
        let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, vec!["Shutdown", "Reboot", "Sleep", "Logout"]);
    }

    #[test]
    fn uses_hyprshutdown_for_session_actions_when_available() {
        let actions = build_actions(true);
        let logout = actions.iter().find(|a| a.label == "Logout").unwrap();
        assert_eq!(logout.command, "hyprshutdown");

        let shutdown = actions.iter().find(|a| a.label == "Shutdown").unwrap();
        assert!(shutdown.command.starts_with("hyprshutdown"));
        assert!(shutdown.command.contains("systemctl poweroff"));

        let reboot = actions.iter().find(|a| a.label == "Reboot").unwrap();
        assert!(reboot.command.starts_with("hyprshutdown"));
        assert!(reboot.command.contains("systemctl reboot"));
    }

    #[test]
    fn falls_back_to_direct_commands_when_hyprshutdown_missing() {
        let actions = build_actions(false);
        let logout = actions.iter().find(|a| a.label == "Logout").unwrap();
        assert_eq!(logout.command, "loginctl terminate-user \"$USER\"");

        let shutdown = actions.iter().find(|a| a.label == "Shutdown").unwrap();
        assert_eq!(shutdown.command, "systemctl poweroff");

        let reboot = actions.iter().find(|a| a.label == "Reboot").unwrap();
        assert_eq!(reboot.command, "systemctl reboot");
    }

    #[test]
    fn sleep_always_uses_direct_suspend_command() {
        for use_hyprshutdown in [true, false] {
            let actions = build_actions(use_hyprshutdown);
            let sleep = actions.iter().find(|a| a.label == "Sleep").unwrap();
            assert_eq!(sleep.command, "systemctl suspend");
        }
    }

    fn labels() -> Vec<String> {
        vec![
            "Shutdown".to_string(),
            "Reboot".to_string(),
            "Sleep".to_string(),
            "Logout".to_string(),
        ]
    }

    #[test]
    fn empty_query_matches_nothing() {
        assert_eq!(best_match_index(&labels(), ""), None);
        assert_eq!(best_match_index(&labels(), "   "), None);
    }

    #[test]
    fn prefix_match_wins_and_is_case_insensitive() {
        assert_eq!(best_match_index(&labels(), "sl"), Some(2));
        assert_eq!(best_match_index(&labels(), "LO"), Some(3));
        assert_eq!(best_match_index(&labels(), "reb"), Some(1));
    }

    #[test]
    fn ties_prefer_earliest_item() {
        // Both "Shutdown" and "Sleep" start with "s"; Shutdown comes first.
        assert_eq!(best_match_index(&labels(), "s"), Some(0));
    }

    #[test]
    fn substring_match_used_when_no_prefix_matches() {
        // Only "Shutdown" contains "d".
        assert_eq!(best_match_index(&labels(), "d"), Some(0));
    }

    #[test]
    fn subsequence_fallback_matches_out_of_order_letters() {
        // "gt" is a subsequence of "Logout" (l-o-g-o-u-t) but not contiguous.
        assert_eq!(best_match_index(&labels(), "gt"), Some(3));
    }

    #[test]
    fn no_match_returns_none() {
        assert_eq!(best_match_index(&labels(), "xyz"), None);
    }
}
