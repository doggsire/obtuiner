use std::{io::stdout, time::Duration};

use anyhow::Result;
use core_domain::PackageRecord;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use runtime_ops::{
    detect_package_managers, install_command, manager_summary, search_packages, uninstall_command,
};
use tui_kit::{
    handle_common_key, handle_modal_key, render_confirm_modal, render_layout,
    CommonAction, ConfirmModal, LayoutData, ModalChoice, SharedState,
};

enum AppMode {
    Browse,
    Confirm(PackageRecord),
    Uninstall(PackageRecord),
}

pub fn run(args: &[String]) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut state = SharedState::default();
    let managers = detect_package_managers();
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let mut mode = AppMode::Browse;
    let mut catalog: Vec<PackageRecord> = Vec::new();
    let mut last_query = String::new();
    let mut last_query_change = std::time::Instant::now();
    let mut search_pending = false;
    let manager_status = format!(" | {}", manager_summary(&managers));
    let status_default = format!(
        "Installer | Tab: focus | Enter: install | Esc: quit{}{}",
        manager_status,
        if dry_run { " [DRY RUN]" } else { "" }
    );
    let mut status = status_default.clone();

    loop {
        // When the query changes, mark a search as pending and start the timer.
        // We wait 300 ms of inactivity before issuing the (potentially slow) search
        // so we don't fire three child processes on every keystroke.
        if state.query != last_query {
            last_query = state.query.clone();
            last_query_change = std::time::Instant::now();
            catalog.clear();
            state.selected = 0;
            search_pending = !state.query.is_empty();
        }
        if search_pending && last_query_change.elapsed().as_millis() >= 300 {
            search_pending = false;
            catalog = search_packages(&state.query, &managers);
            state.selected = 0;
        }

        let filtered = catalog.clone();
        let items: Vec<String> = filtered
            .iter()
            .map(|p| {
                if p.installed {
                    format!("{} [{}] [installed]", p.display_name(), p.source.as_str())
                } else {
                    format!("{} [{}]", p.display_name(), p.source.as_str())
                }
            })
            .collect();

        let details = filtered
            .get(state.selected)
            .map(|pkg| {
                if pkg.installed {
                    vec![
                        format!("Name:   {}", pkg.name),
                        format!("Source: {}", pkg.source.as_str()),
                        format!("Desc:   {}", pkg.description),
                        String::new(),
                        "Status: installed".to_string(),
                        String::new(),
                        "Uninstall command preview:".to_string(),
                        format!("  {}", uninstall_command(pkg, &managers)),
                        String::new(),
                        "Press Enter to uninstall.".to_string(),
                    ]
                } else {
                    vec![
                        format!("Name:   {}", pkg.name),
                        format!("Source: {}", pkg.source.as_str()),
                        format!("Desc:   {}", pkg.description),
                        String::new(),
                        "Install command preview:".to_string(),
                        format!("  {}", install_command(pkg, &managers)),
                        String::new(),
                        "Press Enter to install.".to_string(),
                    ]
                }
            })
            .unwrap_or_else(|| vec!["Type to search packages.".to_string()]);

        terminal.draw(|frame| {
            match &mode {
                AppMode::Confirm(pkg) => {
                    let data = LayoutData {
                        app_title: "Installer",
                        left_title: "Package Results",
                        right_title: "Package Details",
                        status_line: &status,
                        items: &items,
                        details: &details,
                    };
                    render_layout(frame, &state, &data);
                    let modal = ConfirmModal {
                        title: " Confirm Install ".to_string(),
                        lines: vec![
                            format!("Package: {}", pkg.name),
                            format!("Source:  {}", pkg.source.as_str()),
                            String::new(),
                            format!("Command: {}", install_command(pkg, &managers)),
                        ],
                    };
                    render_confirm_modal(frame, &modal);
                }
                AppMode::Uninstall(pkg) => {
                    let data = LayoutData {
                        app_title: "Installer",
                        left_title: "Package Results",
                        right_title: "Package Details",
                        status_line: &status,
                        items: &items,
                        details: &details,
                    };
                    render_layout(frame, &state, &data);
                    let modal = ConfirmModal {
                        title: " Confirm Uninstall ".to_string(),
                        lines: vec![
                            format!("Package: {}", pkg.name),
                            format!("Source:  {}", pkg.source.as_str()),
                            String::new(),
                            format!("Command: {}", uninstall_command(pkg, &managers)),
                        ],
                    };
                    render_confirm_modal(frame, &modal);
                }
                _ => {
                    let data = LayoutData {
                        app_title: "Installer",
                        left_title: "Package Results",
                        right_title: "Package Details",
                        status_line: &status,
                        items: &items,
                        details: &details,
                    };
                    render_layout(frame, &state, &data);
                }
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            if let Event::Key(key) = event {
                match &mode {
                    AppMode::Confirm(pkg) => {
                        let pkg = pkg.clone();
                        match handle_modal_key(key) {
                            ModalChoice::Confirmed => {
                                let cmd = install_command(&pkg, &managers);
                                restore_terminal(&mut terminal)?;
                                if dry_run {
                                    println!("[DRY RUN] Would run: {}", cmd);
                                } else {
                                    println!("Running: {}", cmd);
                                    println!();
                                    let _ = std::process::Command::new("sh")
                                        .args(["-c", &cmd])
                                        .status();
                                }
                                return Ok(());
                            }
                            ModalChoice::Cancelled => {
                                mode = AppMode::Browse;
                                status = status_default.clone();
                            }
                            ModalChoice::Pending => {}
                        }
                    }
                    AppMode::Uninstall(pkg) => {
                        let pkg = pkg.clone();
                        match handle_modal_key(key) {
                            ModalChoice::Confirmed => {
                                let cmd = uninstall_command(&pkg, &managers);
                                restore_terminal(&mut terminal)?;
                                if dry_run {
                                    println!("[DRY RUN] Would run: {}", cmd);
                                } else {
                                    println!("Running: {}", cmd);
                                    println!();
                                    let _ = std::process::Command::new("sh")
                                        .args(["-c", &cmd])
                                        .status();
                                }
                                return Ok(());
                            }
                            ModalChoice::Cancelled => {
                                mode = AppMode::Browse;
                                status = status_default.clone();
                            }
                            ModalChoice::Pending => {}
                        }
                    }
                    _ => {
                        let action = handle_common_key(&mut state, key, filtered.len());
                        match action {
                            CommonAction::Quit => break,
                            CommonAction::Activate => {
                                if let Some(pkg) = filtered.get(state.selected) {
                                    if pkg.installed {
                                        mode = AppMode::Uninstall(pkg.clone());
                                    } else {
                                        mode = AppMode::Confirm(pkg.clone());
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
