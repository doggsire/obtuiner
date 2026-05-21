use std::{collections::HashSet, io::stdout, process::Command, time::Duration};

use anyhow::Result;
use core_domain::{load_profiles, save_profiles, LaunchProfile};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui_kit::
    {handle_common_key, render_layout,
    CommonAction, LayoutData, SharedState};
use runtime_ops::discover_installed_apps;

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

    let mut status = "Launcher | Enter: launch | n: new profile | d: delete | q: quit".to_string();

    loop {
        let filtered = filter_profiles(&profiles, &state.query);
        if filtered.is_empty() {
            state.selected = 0;
        } else {
            state.selected = state.selected.min(filtered.len() - 1);
        }

        let items: Vec<String> = filtered.iter().map(|p| p.name.clone()).collect();
        let details = filtered
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
            .unwrap_or_else(|| vec!["No profile selected.".to_string()]);

        terminal.draw(|frame| {
            let data = LayoutData {
                app_title: "Launcher",
                left_title: "Profiles",
                right_title: "Profile Details",
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
                    KeyCode::Char('d') => {
                        if let Some(profile) = filtered.get(state.selected) {
                            let name = profile.name.clone();
                            profiles.retain(|p| p.name != name);
                            saved_profiles.retain(|p| p.name != name);
                            let _ = save_profiles(&saved_profiles);
                            status = format!("Removed '{}'", name);
                            state.selected = state.selected.saturating_sub(1);
                        }
                    }
                    _ => {
                        let action = handle_common_key(&mut state, key, filtered.len());
                        match action {
                            CommonAction::Quit => break,
                            CommonAction::Activate => {
                                if let Some(profile) = filtered.get(state.selected) {
                                    launch_profile(profile);
                                    break;
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

fn launch_profile(profile: &LaunchProfile) {
    let mut cmd = Command::new(&profile.command);
    cmd.args(&profile.args);
    if !profile.working_dir.is_empty() && profile.working_dir != "~" {
        cmd.current_dir(&profile.working_dir);
    }
    for (key, val) in &profile.env {
        cmd.env(key, val);
    }
    let _ = cmd.spawn();
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
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

