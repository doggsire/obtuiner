use std::{collections::HashSet, io::stdout, os::unix::process::CommandExt, process::Command, time::Duration};

use anyhow::Result;
use core_domain::{load_profiles, save_profiles, LaunchProfile};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_kit::
    {handle_common_key, render_layout,
    CommonAction, LayoutData, SharedState};
use runtime_ops::{discover_installed_apps, discover_path_commands};

// ── Query mode ─────────────────────────────────────────────────────────────

enum QueryMode<'a> {
    /// Normal profile/desktop search.
    Normal,
    /// Command mode: leading `>`. `term` is the text after `>`.
    Command { term: &'a str },
}

fn parse_query(query: &str) -> QueryMode<'_> {
    if let Some(rest) = query.strip_prefix('>') {
        QueryMode::Command { term: rest }
    } else {
        QueryMode::Normal
    }
}

pub fn run(_args: &[String]) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut state = SharedState::default();

    // Load persisted profiles
    let mut saved_profiles = load_profiles().unwrap_or_default();
    if saved_profiles.is_empty() {
        saved_profiles = default_profiles();
        let _ = save_profiles(&saved_profiles);
    }
    // Merge in discovered installed apps (saved profiles take precedence by name)
    let saved_names: HashSet<String> = saved_profiles
        .iter()
        .map(|p| p.name.to_lowercase())
        .collect();
    let mut profiles: Vec<LaunchProfile> = saved_profiles.clone();
    for app in discover_installed_apps() {
        if !saved_names.contains(&app.name.to_lowercase()) {
            profiles.push(app);
        }
    }

    // Discover PATH commands once at startup.
    let path_commands = discover_path_commands();

    let mut status = "Launcher | Enter: launch | n: new profile | d: delete | q: quit | >: command mode".to_string();

    loop {
        let mode = parse_query(&state.query);

        let (filtered_profiles, cmd_candidates): (Vec<LaunchProfile>, Vec<String>) = match mode {
            QueryMode::Normal => (filter_profiles(&profiles, &state.query), vec![]),
            QueryMode::Command { term } => {
                if term.trim().is_empty() {
                    (vec![], vec![])
                } else {
                    let t = term.to_lowercase();
                    let matches = path_commands
                        .iter()
                        .filter(|c| c.to_lowercase().contains(&t))
                        .cloned()
                        .collect();
                    (vec![], matches)
                }
            }
        };

        let is_command_mode = matches!(parse_query(&state.query), QueryMode::Command { .. });
        let result_count = if is_command_mode { cmd_candidates.len() } else { filtered_profiles.len() };

        if result_count == 0 {
            state.selected = 0;
        } else {
            state.selected = state.selected.min(result_count - 1);
        }

        let items: Vec<String> = if is_command_mode {
            cmd_candidates.clone()
        } else {
            filtered_profiles.iter().map(|p| p.name.clone()).collect()
        };

        let details: Vec<String> = if is_command_mode {
            if let Some(name) = cmd_candidates.get(state.selected) {
                vec![
                    format!("Command:  {}", name),
                    String::new(),
                    "Press Enter to run.".to_string(),
                ]
            } else {
                vec!["No command selected.".to_string()]
            }
        } else {
            filtered_profiles
                .get(state.selected)
                .map(|p| {
                    let env_str = if p.env.is_empty() {
                        "(none)".to_string()
                    } else {
                        p.env.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(", ")
                    };
                    vec![
                        format!("Profile:  {}", p.name),
                        format!("Command:  {}", p.command),
                        format!("Args:     {}", p.args.join(" ")),
                        format!("Dir:      {}", p.working_dir),
                        format!("Env:      {}", env_str),
                        String::new(),
                        "Press Enter to launch.".to_string(),
                    ]
                })
                .unwrap_or_else(|| vec!["No profile selected.".to_string()])
        };

        let left_title = if is_command_mode { "Commands (PATH)" } else { "Profiles" };
        let right_title = if is_command_mode { "Command Details" } else { "Profile Details" };

        terminal.draw(|frame| {
            let data = LayoutData {
                app_title: "Launcher",
                left_title,
                right_title,
                status_line: &status,
                items: &items,
                details: &details,
            };
            render_layout(frame, &state, &data);
        })?;

        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('d') if !is_command_mode => {
                        if let Some(profile) = filtered_profiles.get(state.selected) {
                            let name = profile.name.clone();
                            profiles.retain(|p| p.name != name);
                            saved_profiles.retain(|p| p.name != name);
                            let _ = save_profiles(&saved_profiles);
                            status = format!("Removed '{}'", name);
                            state.selected = state.selected.saturating_sub(1);
                        }
                    }
                    _ => {
                        let action = handle_common_key(&mut state, key, result_count);
                        match action {
                            CommonAction::Quit => break,
                            CommonAction::Activate => {
                                if is_command_mode {
                                    if let Some(cmd_name) = cmd_candidates.get(state.selected) {
                                        match launch_command_line(cmd_name) {
                                            Ok(()) => break,
                                            Err(e) => status = format!("Launch failed: {}", e),
                                        }
                                    }
                                } else if let Some(profile) = filtered_profiles.get(state.selected) {
                                    match launch_profile(profile) {
                                        Ok(()) => break,
                                        Err(e) => status = format!("Launch failed: {}", e),
                                    }
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

fn launch_profile(profile: &LaunchProfile) -> Result<()> {
    let mut cmd = Command::new(&profile.command);
    cmd.args(&profile.args);
    cmd.process_group(0);
    if !profile.working_dir.is_empty() && profile.working_dir != "~" {
        cmd.current_dir(&profile.working_dir);
    }
    for (key, val) in &profile.env {
        cmd.env(key, val);
    }
    cmd.spawn().map(|_| ()).map_err(|e| anyhow::anyhow!("'{}': {}", profile.command, e))
}

/// Run a command-mode line: split on whitespace into argv and exec the first token.
fn launch_command_line(line: &str) -> Result<()> {
    let mut tokens = line.split_whitespace();
    let command = tokens
        .next()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| anyhow::anyhow!("empty command"))?;
    let args: Vec<&str> = tokens.collect();
    let mut cmd = Command::new(command);
    cmd.args(&args);
    cmd.process_group(0);
    cmd.spawn().map(|_| ()).map_err(|e| anyhow::anyhow!("'{}': {}", command, e))
}

fn default_profiles() -> Vec<LaunchProfile> {
    vec![
        LaunchProfile {
            name: "Terminal".to_string(),
            command: "xterm".to_string(),
            args: vec![],
            env: vec![],
            working_dir: String::new(),
        },
        LaunchProfile {
            name: "VS Code".to_string(),
            command: "code".to_string(),
            args: vec![],
            env: vec![],
            working_dir: String::new(),
        },
    ]
}

fn filter_profiles(items: &[LaunchProfile], query: &str) -> Vec<LaunchProfile> {
    if query.trim().is_empty() {
        return items.to_vec();
    }
    let q = query.to_lowercase();
    items
        .iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&q) || p.command.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
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
    execute!(terminal.backend_mut(), Clear(ClearType::All), MoveTo(0, 0), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_query, QueryMode};

    #[test]
    fn empty_query_is_normal_mode() {
        assert!(matches!(parse_query(""), QueryMode::Normal));
    }

    #[test]
    fn plain_text_is_normal_mode() {
        assert!(matches!(parse_query("firefox"), QueryMode::Normal));
    }

    #[test]
    fn gt_prefix_enters_command_mode() {
        let mode = parse_query(">echo");
        assert!(matches!(mode, QueryMode::Command { term: "echo" }));
    }

    #[test]
    fn bare_gt_has_empty_term() {
        let mode = parse_query(">");
        assert!(matches!(mode, QueryMode::Command { term: "" }));
    }

    #[test]
    fn command_mode_term_includes_args() {
        let mode = parse_query(">echo hello world");
        assert!(matches!(mode, QueryMode::Command { term: "echo hello world" }));
    }
}


