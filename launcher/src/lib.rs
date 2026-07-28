use std::{collections::{HashMap, HashSet}, io::stdout, os::unix::process::CommandExt, process::Command, time::Duration};

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
    CommonAction, FocusPane, LayoutData, SharedState};
use runtime_ops::{discover_installed_apps, discover_path_commands, get_command_flags, complete_path_prefix};

// ── Query mode ─────────────────────────────────────────────────────────────

enum QueryMode<'a> {
    /// Normal profile/desktop search.
    Normal,
    /// Command mode: no space yet after `>` — still picking the command name.
    /// `term` is the text after `>`.
    Command { term: &'a str },
    /// Command is locked in; user is building arguments.
    /// `cmd` is the chosen command, `arg_prefix` is everything after the first space.
    CommandArgs { cmd: &'a str, arg_prefix: &'a str },
}

fn parse_query(query: &str) -> QueryMode<'_> {
    if let Some(rest) = query.strip_prefix('>') {
        // Is there a space after the command name?
        if let Some(space_pos) = rest.find(' ') {
            let cmd = &rest[..space_pos];
            let arg_prefix = &rest[space_pos + 1..];
            QueryMode::CommandArgs { cmd, arg_prefix }
        } else {
            QueryMode::Command { term: rest }
        }
    } else {
        QueryMode::Normal
    }
}

pub fn run(_args: &[String]) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut state = SharedState::default();

    // Load persisted profiles
    let mut saved_profiles = load_profiles().unwrap_or_default();
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

    // Cache flag completions per command so we don't spawn --help every frame.
    let mut cmd_flags_cache: HashMap<String, Vec<String>> = HashMap::new();

    let mut status = "Launcher | Enter: launch | n: new profile | d: delete | Esc: quit | >: command mode".to_string();

    loop {
        let mode = parse_query(&state.query);

        // ── Build display items based on mode ──────────────────────────────
        let (filtered_profiles, cmd_candidates, arg_candidates): (Vec<LaunchProfile>, Vec<String>, Vec<String>) = match mode {
            QueryMode::Normal => (filter_profiles(&profiles, &state.query), vec![], vec![]),
            QueryMode::Command { term } => {
                if term.trim().is_empty() {
                    (vec![], vec![], vec![])
                } else {
                    let t = term.to_lowercase();
                    let mut matches: Vec<String> = path_commands
                        .iter()
                        .filter(|c| c.to_lowercase().contains(&t))
                        .cloned()
                        .collect();
                    matches.sort_by(|a, b| {
                        let ra = match_rank(a, &t);
                        let rb = match_rank(b, &t);
                        ra.cmp(&rb).then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
                    });
                    (vec![], matches, vec![])
                }
            }
            QueryMode::CommandArgs { cmd, arg_prefix } => {
                // Complete only the last whitespace-separated token being typed.
                let last_token = if arg_prefix.ends_with(|c: char| c.is_whitespace()) {
                    ""
                } else {
                    arg_prefix.split_whitespace().last().unwrap_or("")
                };
                let completions = if last_token.starts_with('-') {
                    // Flag completion — use per-command cache so --help is only
                    // spawned once per command, not on every render frame.
                    let flags = cmd_flags_cache
                        .entry(cmd.to_string())
                        .or_insert_with(|| get_command_flags(cmd));
                    let p = last_token.to_lowercase();
                    flags.iter()
                        .filter(|f| f.to_lowercase().starts_with(&p))
                        .cloned()
                        .collect()
                } else {
                    complete_path_prefix(last_token)
                };
                (vec![], vec![], completions)
            }
        };

        let is_command_mode = matches!(parse_query(&state.query), QueryMode::Command { .. });
        let is_args_mode = matches!(parse_query(&state.query), QueryMode::CommandArgs { .. });

        let result_count = if is_args_mode {
            arg_candidates.len()
        } else if is_command_mode {
            cmd_candidates.len()
        } else {
            filtered_profiles.len()
        };

        if result_count == 0 {
            state.selected = 0;
        } else {
            state.selected = state.selected.min(result_count - 1);
        }

        let items: Vec<String> = if is_args_mode {
            arg_candidates.clone()
        } else if is_command_mode {
            cmd_candidates.clone()
        } else {
            filtered_profiles.iter().map(|p| p.name.clone()).collect()
        };

        let details: Vec<String> = if is_args_mode {
            if let QueryMode::CommandArgs { cmd, arg_prefix } = parse_query(&state.query) {
                let (before, last_token) = split_last_token(arg_prefix);
                let preview = arg_candidates
                    .get(state.selected)
                    .map(|s| s.as_str())
                    .unwrap_or(last_token);
                let will_run = format!("{} {}{}", cmd, before, preview);
                vec![
                    format!("Command:  {}", cmd),
                    format!("Args:     {}{}", before, preview),
                    String::new(),
                    format!("Will run: {}", will_run.trim_end()),
                    String::new(),
                    "Space: complete token  |  Enter: run command".to_string(),
                ]
            } else {
                vec![]
            }
        } else if is_command_mode {
            if let Some(name) = cmd_candidates.get(state.selected) {
                vec![
                    format!("Command:  {}", name),
                    String::new(),
                    "Space: complete & add args  |  Enter: run now".to_string(),
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

        let left_title = if is_args_mode {
            "Arg Completions"
        } else if is_command_mode {
            "Commands (PATH)"
        } else {
            "Profiles"
        };
        let right_title = if is_args_mode || is_command_mode { "Command Details" } else { "Profile Details" };

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
                    KeyCode::Char('d') if !is_command_mode && !is_args_mode && state.focus != FocusPane::Search => {
                        if let Some(profile) = filtered_profiles.get(state.selected) {
                            let name = profile.name.clone();
                            profiles.retain(|p| p.name != name);
                            saved_profiles.retain(|p| p.name != name);
                            let _ = save_profiles(&saved_profiles);
                            status = format!("Removed '{}'", name);
                            state.selected = state.selected.saturating_sub(1);
                        }
                    }
                    // Space in the search bar completes the top candidate:
                    // - command name mode  → completes command, enters args mode
                    // - args mode with a completion available → splices the top
                    //   completion in place of the last token, then appends a space
                    //   so the next arg can be typed immediately.
                    // - args mode with no completions → falls through as literal space.
                    KeyCode::Char(' ') if state.focus == FocusPane::Search && (is_command_mode || is_args_mode) => {
                        if is_command_mode {
                            if let Some(cmd_name) = cmd_candidates.first() {
                                state.query = format!(">{} ", cmd_name);
                                state.selected = 0;
                                status = format!("Command: {} — type args, use ↓ to pick completions", cmd_name);
                            }
                        } else if is_args_mode {
                            if let QueryMode::CommandArgs { cmd, arg_prefix } = parse_query(&state.query) {
                                if let Some(completion) = arg_candidates.first() {
                                    let (before, _) = split_last_token(arg_prefix);
                                    // Directories keep no trailing space so you can
                                    // drill down; everything else gets one.
                                    let suffix = if completion.ends_with('/') { "" } else { " " };
                                    state.query = format!(">{} {}{}{}", cmd, before, completion, suffix);
                                    state.selected = 0;
                                } else {
                                    // No completions — insert a literal space so the
                                    // user can keep typing the next arg by hand.
                                    state.query.push(' ');
                                    state.selected = 0;
                                }
                            }
                        }
                    }
                    _ => {
                        let action = handle_common_key(&mut state, key, result_count);
                        match action {
                            CommonAction::Quit => break,
                            CommonAction::CompleteSelected => {
                                if is_command_mode {
                                    // Complete the command name: set query to ">cmd " and focus search
                                    if let Some(cmd_name) = cmd_candidates.get(state.selected) {
                                        state.query = format!(">{} ", cmd_name);
                                        state.selected = 0;
                                        state.focus = FocusPane::Search;
                                        status = format!("Command: {} — type args, use ↓ to pick completions", cmd_name);
                                    }
                                } else if is_args_mode {
                                    // Replace only the last token with the selected completion.
                                    // Everything typed before it (earlier args) is kept.
                                    if let QueryMode::CommandArgs { cmd, arg_prefix } = parse_query(&state.query) {
                                        if let Some(completion) = arg_candidates.get(state.selected) {
                                            let (before, _) = split_last_token(arg_prefix);
                                            // Append a trailing space only for non-directories so
                                            // the next arg can be typed straight away.
                                            let suffix = if completion.ends_with('/') { "" } else { " " };
                                            state.query = format!(">{} {}{}{}", cmd, before, completion, suffix);
                                            state.selected = 0;
                                            state.focus = FocusPane::Search;
                                        }
                                    }
                                }
                            }
                            CommonAction::Activate => {
                                if is_args_mode {
                                    let line = if let QueryMode::CommandArgs { cmd, arg_prefix } = parse_query(&state.query) {
                                        if state.focus == FocusPane::Results {
                                            // Apply the highlighted completion for the last token, then run.
                                            if let Some(completion) = arg_candidates.get(state.selected) {
                                                let (before, _) = split_last_token(arg_prefix);
                                                format!("{} {}{}", cmd, before, completion)
                                            } else {
                                                format!("{} {}", cmd, arg_prefix)
                                            }
                                        } else {
                                            // Run whatever is currently in the search bar as-is.
                                            format!("{} {}", cmd, arg_prefix)
                                        }
                                    } else {
                                        String::new()
                                    };
                                    if !line.is_empty() {
                                        match launch_command_line_shell(line.trim_end()) {
                                            Ok(()) => break,
                                            Err(e) => status = format!("Launch failed: {}", e),
                                        }
                                    }
                                } else if is_command_mode {
                                    if let Some(cmd_name) = cmd_candidates.get(state.selected) {
                                        match launch_command_line_shell(cmd_name) {
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

/// Run a command line through the user's login shell so shell builtins,
/// aliases, functions, and PATH expansions all work as expected.
fn launch_command_line_shell(line: &str) -> Result<()> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = Command::new(&shell);
    cmd.args(["-c", line]);
    cmd.process_group(0);
    cmd.spawn().map(|_| ()).map_err(|e| anyhow::anyhow!("'{}': {}", line, e))
}

/// Split `args` into the text before the last token and the last token itself.
/// If `args` ends with whitespace, the last token is empty (new arg being started).
fn split_last_token(args: &str) -> (&str, &str) {
    if args.ends_with(|c: char| c.is_whitespace()) {
        (args, "")
    } else {
        match args.rfind(|c: char| c.is_whitespace()) {
            Some(pos) => (&args[..=pos], &args[pos + 1..]),
            None => ("", args),
        }
    }
}

fn match_rank(haystack: &str, needle: &str) -> u8 {
    let h = haystack.to_lowercase();
    if h == needle { 0 }
    else if h.starts_with(needle) { 1 }
    else { 2 }
}

fn filter_profiles(items: &[LaunchProfile], query: &str) -> Vec<LaunchProfile> {
    if query.trim().is_empty() {
        return items.to_vec();
    }
    let q = query.to_lowercase();
    let mut results: Vec<LaunchProfile> = items
        .iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&q) || p.command.to_lowercase().contains(&q)
        })
        .cloned()
        .collect();
    results.sort_by(|a, b| {
        let ra = match_rank(&a.name, &q).min(match_rank(&a.command, &q));
        let rb = match_rank(&b.name, &q).min(match_rank(&b.command, &q));
        ra.cmp(&rb).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    results
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
    fn command_mode_term_no_space() {
        let mode = parse_query(">echo");
        assert!(matches!(mode, QueryMode::Command { term: "echo" }));
    }

    #[test]
    fn command_args_mode_with_space() {
        let mode = parse_query(">echo hello world");
        assert!(matches!(mode, QueryMode::CommandArgs { cmd: "echo", arg_prefix: "hello world" }));
    }
}


