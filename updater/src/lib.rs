use std::{io::stdout, time::Duration};

use anyhow::Result;
use core_domain::{PackageRecord, PackageSource, UpdaterTarget};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use runtime_ops::{
    check_apt_updates, check_dnf_updates, check_flatpak_updates, check_pacman_updates,
    check_zypper_updates, detect_package_managers, full_upgrade_commands, manager_summary,
    package_update_command, ManagerContext, NativeManager,
};
use tui_kit::{
    handle_common_key, handle_modal_key, render_confirm_modal, render_layout,
    CommonAction, ConfirmModal, LayoutData, ModalChoice, SharedState,
};

#[derive(Clone)]
struct UpdateEntry {
    label: String,
    target: UpdaterTarget,
}

enum AppMode {
    Browse,
    Confirm(UpdateEntry),
}

pub fn run(args: &[String]) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut state = SharedState::default();
    let managers = detect_package_managers();
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let mut mode = AppMode::Browse;
    let entries = build_entries(&managers);
    let mut status = format!(
        "Updater | Default: full system upgrade | {}{}",
        manager_summary(&managers),
        if dry_run { " [DRY RUN]" } else { "" }
    );

    loop {
        // Refresh entries when search changes
        let filtered = filter_entries(&entries, &state.query);
        if filtered.is_empty() {
            state.selected = 0;
        } else {
            state.selected = state.selected.min(filtered.len() - 1);
        }

        let items: Vec<String> = filtered.iter().map(|e| e.label.clone()).collect();
        let details = filtered
            .get(state.selected)
            .map(|e| detail_lines(e, &managers))
            .unwrap_or_else(|| vec!["No task selected".to_string()]);

        terminal.draw(|frame| {
            let data = LayoutData {
                app_title: "Updater",
                left_title: "Update Tasks",
                right_title: "Task Details",
                status_line: &status,
                items: &items,
                details: &details,
            };
            match &mode {
                AppMode::Confirm(entry) => {
                    render_layout(frame, &state, &data);
                    let (title, cmd_lines) = modal_content(entry, &managers);
                    render_confirm_modal(frame, &ConfirmModal { title, lines: cmd_lines });
                }
                AppMode::Browse => render_layout(frame, &state, &data),
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            let event = event::read()?;
            if let Event::Key(key) = event {
                match &mode {
                    AppMode::Confirm(entry) => {
                        let entry = entry.clone();
                        match handle_modal_key(key) {
                            ModalChoice::Confirmed => {
                                let cmds = execution_commands(&entry, &managers);
                                restore_terminal(&mut terminal)?;
                                if dry_run {
                                    for cmd in &cmds {
                                        println!("[DRY RUN] Would run: {}", cmd);
                                    }
                                } else {
                                    for cmd in &cmds {
                                        println!();
                                        println!("Running: {}", cmd);
                                        println!();
                                        let ok = std::process::Command::new("sh")
                                            .args(["-c", cmd])
                                            .status()
                                            .map(|s| s.success())
                                            .unwrap_or(false);
                                        if !ok {
                                            println!("Command failed — stopping.");
                                            break;
                                        }
                                    }
                                }
                                return Ok(());
                            }
                            ModalChoice::Cancelled => {
                                mode = AppMode::Browse;
                                status = format!(
                                    "Updater | Default: full system upgrade | {}",
                                    manager_summary(&managers)
                                );
                            }
                            ModalChoice::Pending => {}
                        }
                    }
                    AppMode::Browse => {
                        let action = handle_common_key(&mut state, key, filtered.len());
                        match action {
                            CommonAction::Quit => break,
                            CommonAction::Activate => {
                                if let Some(entry) = filtered.get(state.selected) {
                                    mode = AppMode::Confirm(entry.clone());
                                }
                            }
                            CommonAction::None | CommonAction::CompleteSelected => {}
                        }
                    }
                }
            }
        }
    }

    restore_terminal(&mut terminal)
}

fn build_entries(managers: &ManagerContext) -> Vec<UpdateEntry> {
    let mut entries = vec![UpdateEntry {
        label: "  Full system upgrade (default)".to_string(),
        target: UpdaterTarget::FullSystem,
    }];

    match &managers.native {
        Some(NativeManager::Pacman) => {
            for name in check_pacman_updates() {
                entries.push(UpdateEntry {
                    label: format!("[pacman] {}", name),
                    target: UpdaterTarget::Package(PackageRecord {
                        name: name.clone(),
                        source: PackageSource::Pacman,
                        description: String::new(),
                        installed: true,
                    }),
                });
            }
            if let Some(h) = managers.aur_helper.as_deref() {
                entries.push(UpdateEntry {
                    label: format!("[aur] Upgrade all AUR packages ({})", h),
                    target: UpdaterTarget::Package(PackageRecord {
                        name: "__AUR_ALL__".to_string(),
                        source: PackageSource::Aur,
                        description: "Upgrade all outdated AUR packages".to_string(),
                        installed: true,
                    }),
                });
            }
        }
        Some(NativeManager::Apt { helper }) => {
            for name in check_apt_updates() {
                entries.push(UpdateEntry {
                    label: format!("[{}] {}", helper, name),
                    target: UpdaterTarget::Package(PackageRecord {
                        name: name.clone(),
                        source: PackageSource::Apt,
                        description: String::new(),
                        installed: true,
                    }),
                });
            }
        }
        Some(NativeManager::Dnf) => {
            for name in check_dnf_updates() {
                entries.push(UpdateEntry {
                    label: format!("[dnf] {}", name),
                    target: UpdaterTarget::Package(PackageRecord {
                        name: name.clone(),
                        source: PackageSource::Dnf,
                        description: String::new(),
                        installed: true,
                    }),
                });
            }
        }
        Some(NativeManager::Zypper) => {
            for name in check_zypper_updates() {
                entries.push(UpdateEntry {
                    label: format!("[zypper] {}", name),
                    target: UpdaterTarget::Package(PackageRecord {
                        name: name.clone(),
                        source: PackageSource::Zypper,
                        description: String::new(),
                        installed: true,
                    }),
                });
            }
        }
        None => {}
    }

    // Add flatpak updates
    if managers.flatpak_available {
        for id in check_flatpak_updates() {
            entries.push(UpdateEntry {
                label: format!("[flatpak] {}", id),
                target: UpdaterTarget::Package(PackageRecord {
                    name: id.clone(),
                    source: PackageSource::Flatpak,
                    description: String::new(),
                    installed: true,
                }),
            });
        }
    }

    entries
}

fn filter_entries(items: &[UpdateEntry], query: &str) -> Vec<UpdateEntry> {
    if query.trim().is_empty() {
        return items.to_vec();
    }
    let q = query.to_lowercase();
    items
        .iter()
        .filter(|e| e.label.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

fn detail_lines(entry: &UpdateEntry, managers: &ManagerContext) -> Vec<String> {
    match &entry.target {
        UpdaterTarget::FullSystem => {
            let cmds = full_upgrade_commands(managers);
            let mut lines = vec!["Default full upgrade order:".to_string()];
            for (i, cmd) in cmds.iter().enumerate() {
                lines.push(format!("  {}) {}", i + 1, cmd));
            }
            lines.push(String::new());
            lines.push("Press Enter to confirm this sequence.".to_string());
            lines
        }
        UpdaterTarget::Package(pkg) => vec![
            format!("Package: {}", pkg.name),
            format!("Source:  {}", pkg.source.as_str()),
            format!("Desc:    {}", pkg.description),
            String::new(),
            "Update command:".to_string(),
            format!("  {}", package_update_command(pkg, managers)),
        ],
    }
}

fn modal_content(entry: &UpdateEntry, managers: &ManagerContext) -> (String, Vec<String>) {
    match &entry.target {
        UpdaterTarget::FullSystem => {
            let cmds = full_upgrade_commands(managers);
            (
                " Confirm Full System Upgrade ".to_string(),
                cmds
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{}) {}", i + 1, c))
                    .collect(),
            )
        }
        UpdaterTarget::Package(pkg) => (
            " Confirm Package Update ".to_string(),
            vec![
                format!("Package: {}", pkg.name),
                format!("Command: {}", package_update_command(pkg, managers)),
            ],
        ),
    }
}

fn execution_commands(entry: &UpdateEntry, managers: &ManagerContext) -> Vec<String> {
    match &entry.target {
        UpdaterTarget::FullSystem => full_upgrade_commands(managers),
        UpdaterTarget::Package(pkg) => vec![package_update_command(pkg, managers)],
    }
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


