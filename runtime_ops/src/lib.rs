use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use core_domain::{LaunchProfile, PackageRecord, PackageSource};

// ── AUR helper detection ───────────────────────────────────────────────────

pub fn detect_aur_helper() -> Option<String> {
    ["paru", "yay"].iter().find_map(|candidate| {
        let status = Command::new("which")
            .arg(candidate)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()?;
        if status.success() {
            Some((*candidate).to_string())
        } else {
            None
        }
    })
}

#[derive(Clone, Debug)]
pub enum NativeManager {
    Pacman,
    Apt { helper: String },
    Dnf,
    Zypper,
}

#[derive(Clone, Debug)]
pub struct ManagerContext {
    pub native: Option<NativeManager>,
    pub aur_helper: Option<String>,
    pub flatpak_available: bool,
}

fn command_exists(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn detect_native_manager() -> Option<NativeManager> {
    if command_exists("pacman") {
        Some(NativeManager::Pacman)
    } else if command_exists("nala") {
        Some(NativeManager::Apt {
            helper: "nala".to_string(),
        })
    } else if command_exists("apt") {
        Some(NativeManager::Apt {
            helper: "apt".to_string(),
        })
    } else if command_exists("dnf") {
        Some(NativeManager::Dnf)
    } else if command_exists("zypper") {
        Some(NativeManager::Zypper)
    } else {
        None
    }
}

pub fn detect_package_managers() -> ManagerContext {
    let native = detect_native_manager();
    let aur_helper = if matches!(native, Some(NativeManager::Pacman)) {
        detect_aur_helper()
    } else {
        None
    };
    ManagerContext {
        native,
        aur_helper,
        flatpak_available: command_exists("flatpak"),
    }
}

pub fn manager_summary(managers: &ManagerContext) -> String {
    let native = match &managers.native {
        Some(NativeManager::Pacman) => "native: pacman".to_string(),
        Some(NativeManager::Apt { helper }) => format!("native: {}", helper),
        Some(NativeManager::Dnf) => "native: dnf".to_string(),
        Some(NativeManager::Zypper) => "native: zypper".to_string(),
        None => "native: none".to_string(),
    };
    let aur = match &managers.aur_helper {
        Some(h) => format!("aur: {}", h),
        None => "aur: none".to_string(),
    };
    let flatpak = if managers.flatpak_available {
        "flatpak: yes"
    } else {
        "flatpak: no"
    };
    format!("{} | {} | {}", native, aur, flatpak)
}

// ── Command string builders ────────────────────────────────────────────────

pub fn install_command(pkg: &PackageRecord, managers: &ManagerContext) -> String {
    match pkg.source {
        PackageSource::Pacman => format!("sudo pacman -S --needed --noconfirm {}", pkg.name),
        PackageSource::Aur => {
            let helper = managers.aur_helper.as_deref().unwrap_or("paru");
            format!("{} -S --noconfirm {}", helper, pkg.name)
        }
        PackageSource::Apt => {
            let helper = match &managers.native {
                Some(NativeManager::Apt { helper }) => helper.as_str(),
                _ => "apt",
            };
            format!("sudo {} install -y {}", helper, pkg.name)
        }
        PackageSource::Dnf => format!("sudo dnf install -y {}", pkg.name),
        PackageSource::Zypper => format!("sudo zypper install -y {}", pkg.name),
        PackageSource::Flatpak => format!("flatpak install -y flathub {}", pkg.name),
    }
}

pub fn uninstall_command(pkg: &PackageRecord, managers: &ManagerContext) -> String {
    match pkg.source {
        PackageSource::Pacman | PackageSource::Aur => {
            format!("sudo pacman -Rns --noconfirm {}", pkg.name)
        }
        PackageSource::Apt => {
            let helper = match &managers.native {
                Some(NativeManager::Apt { helper }) => helper.as_str(),
                _ => "apt",
            };
            format!("sudo {} remove -y {}", helper, pkg.name)
        }
        PackageSource::Dnf => format!("sudo dnf remove -y {}", pkg.name),
        PackageSource::Zypper => format!("sudo zypper remove -y {}", pkg.name),
        PackageSource::Flatpak => format!("flatpak uninstall -y {}", pkg.name),
    }
}

pub fn package_update_command(pkg: &PackageRecord, managers: &ManagerContext) -> String {
    match pkg.source {
        PackageSource::Pacman => format!("sudo pacman -S --noconfirm {}", pkg.name),
        PackageSource::Aur => {
            let helper = managers.aur_helper.as_deref().unwrap_or("paru");
            if pkg.name == "__AUR_ALL__" {
                format!("{} -Sua --noconfirm", helper)
            } else {
                format!("{} -S --noconfirm {}", helper, pkg.name)
            }
        }
        PackageSource::Apt => {
            let helper = match &managers.native {
                Some(NativeManager::Apt { helper }) => helper.as_str(),
                _ => "apt",
            };
            format!("sudo {} install -y --only-upgrade {}", helper, pkg.name)
        }
        PackageSource::Dnf => format!("sudo dnf upgrade -y {}", pkg.name),
        PackageSource::Zypper => format!("sudo zypper update -y {}", pkg.name),
        PackageSource::Flatpak => format!("flatpak update -y {}", pkg.name),
    }
}

/// Returns the three default full upgrade commands in order:
/// 1) pacman  2) AUR helper  3) flatpak
pub fn full_upgrade_commands(managers: &ManagerContext) -> Vec<String> {
    let mut cmds = Vec::new();
    match &managers.native {
        Some(NativeManager::Pacman) => {
            cmds.push("sudo pacman -Syu --noconfirm".to_string());
            if let Some(helper) = managers.aur_helper.as_deref() {
                cmds.push(format!("{} -Sua --noconfirm", helper));
            }
        }
        Some(NativeManager::Apt { helper }) => {
            cmds.push(format!("sudo {} update", helper));
            cmds.push(format!("sudo {} upgrade -y", helper));
        }
        Some(NativeManager::Dnf) => {
            cmds.push("sudo dnf upgrade -y".to_string());
        }
        Some(NativeManager::Zypper) => {
            cmds.push("sudo zypper update -y".to_string());
        }
        None => {}
    }
    if managers.flatpak_available {
        cmds.push("flatpak update -y".to_string());
    }
    cmds
}

// ── Execution result ───────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ExecResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub dry_run: bool,
}

impl ExecResult {
    pub fn summary(&self) -> String {
        if self.dry_run {
            format!("[DRY RUN] would execute: {}", self.command)
        } else if self.success {
            format!("[OK] {}", self.command)
        } else {
            format!(
                "[FAILED] {}\nstderr: {}",
                self.command,
                self.stderr.trim()
            )
        }
    }
}

// ── Shell command runner ───────────────────────────────────────────────────

/// Run a shell command optionally in dry-run mode.
/// In dry-run mode the command string is logged but never executed.
pub fn run_command(command_str: &str, dry_run: bool) -> Result<ExecResult> {
    if dry_run {
        return Ok(ExecResult {
            command: command_str.to_string(),
            stdout: String::new(),
            stderr: String::new(),
            success: true,
            dry_run: true,
        });
    }

    let output = Command::new("sh")
        .arg("-c")
        .arg(command_str)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to spawn command: {}", command_str))?;

    Ok(ExecResult {
        command: command_str.to_string(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
        dry_run: false,
    })
}

/// Run a sequence of commands, stopping on first failure.
pub fn run_sequence(commands: &[String], dry_run: bool) -> Vec<ExecResult> {
    let mut results = Vec::new();
    for cmd in commands {
        let result = match run_command(cmd, dry_run) {
            Ok(r) => r,
            Err(e) => ExecResult {
                command: cmd.clone(),
                stdout: String::new(),
                stderr: e.to_string(),
                success: false,
                dry_run,
            },
        };
        let ok = result.success;
        results.push(result);
        if !ok {
            break;
        }
    }
    results
}

// ── Package discovery ──────────────────────────────────────────────────────

/// Query pacman for all installed packages matching query.
pub fn query_pacman(query: &str) -> Vec<PackageRecord> {
    let output = Command::new("pacman")
        .args(["-Ss", query])
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });

    parse_pacman_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_pacman_output(raw: &str) -> Vec<PackageRecord> {
    let mut records = Vec::new();
    let mut lines = raw.lines().peekable();
    while let Some(line) = lines.next() {
        // pacman -Ss format: "repo/name version [installed]"  then indented description
        if line.starts_with(' ') || line.is_empty() {
            continue;
        }
        let name = line.split_whitespace().next().unwrap_or("").to_string();
        // Strip "repo/" prefix
        let name = name.split('/').last().unwrap_or(&name).to_string();
        // "[installed]" or "[installed: <version>]" anywhere in the header line
        let installed = line.contains("[installed");
        let description = lines
            .peek()
            .filter(|l| l.starts_with("    "))
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        if !name.is_empty() {
            records.push(PackageRecord {
                name,
                source: PackageSource::Pacman,
                description,
                installed,
            });
        }
    }
    records
}

/// Query AUR helper for packages matching query.
pub fn query_aur(query: &str, aur_helper: &str) -> Vec<PackageRecord> {
    let output = Command::new(aur_helper)
        .args(["-Ss", query])
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });

    // Parse with same format as pacman then relabel as Aur
    parse_pacman_output(&String::from_utf8_lossy(&output.stdout))
        .into_iter()
        .map(|mut r| {
            r.source = PackageSource::Aur;
            r
        })
        .collect()
}

/// Query apt/nala package cache for packages matching query.
pub fn query_apt(query: &str) -> Vec<PackageRecord> {
    let output = Command::new("apt-cache")
        .args(["search", query])
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (name, desc) = line.split_once(" - ")?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some(PackageRecord {
                name: name.to_string(),
                source: PackageSource::Apt,
                description: desc.trim().to_string(),
                installed: false,
            })
        })
        .collect()
}

/// Return the set of currently-installed RPM package names (used by DNF systems).
fn rpm_installed_set() -> std::collections::HashSet<String> {
    let output = Command::new("rpm")
        .args(["-qa", "--queryformat", "%{NAME}\n"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = output else {
        return std::collections::HashSet::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Query dnf for packages matching query.
pub fn query_dnf(query: &str) -> Vec<PackageRecord> {
    let output = Command::new("dnf")
        .args(["search", "--quiet", query])
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });

    let installed = rpm_installed_set();

    // DNF search --quiet output format (Fedora):
    //   Matched fields: name (exact), summary
    //    firefox.x86_64 Mozilla Firefox Web browser
    //    browserpass-firefox.x86_64     Native component for the Firefox extension
    // Each result line is indented; header lines start with "Matched fields".
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            // Skip section headers and blank lines
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with("Matched fields") {
                return None;
            }
            // Split on first whitespace: "name.arch  description"
            let (name_arch, desc) = trimmed.split_once(' ')?;
            // Strip the trailing architecture suffix (last ".component")
            let name = match name_arch.rsplit_once('.') {
                Some((base, _arch)) => base,
                None => name_arch,
            };
            if name.is_empty() {
                return None;
            }
            let is_installed = installed.contains(name);
            Some(PackageRecord {
                name: name.to_string(),
                source: PackageSource::Dnf,
                description: desc.trim().to_string(),
                installed: is_installed,
            })
        })
        .collect()
}

/// Query zypper for packages matching query.
pub fn query_zypper(query: &str) -> Vec<PackageRecord> {
    let output = Command::new("zypper")
        .args(["search", query])
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            if !line.contains('|') || line.starts_with('-') {
                return None;
            }
            let cols: Vec<&str> = line.split('|').map(|c| c.trim()).collect();
            if cols.len() < 3 {
                return None;
            }
            let name = cols[1];
            let description = cols[2];
            if name.is_empty() || name.eq_ignore_ascii_case("Name") {
                return None;
            }
            Some(PackageRecord {
                name: name.to_string(),
                source: PackageSource::Zypper,
                description: description.to_string(),
                installed: false,
            })
        })
        .collect()
}

/// Query flatpak for available apps matching query.
pub fn query_flatpak(query: &str) -> Vec<PackageRecord> {
    let output = Command::new("flatpak")
        // "application" and "description" are supported by all flatpak versions
        .args(["search", "--columns=application,description", query])
        .output()
        .unwrap_or_else(|_| std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });

    // Build a set of installed flatpak app IDs so we can mark results.
    let installed_flatpaks = flatpak_installed_set();

    let raw = String::from_utf8_lossy(&output.stdout);
    let mut records = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let cols = split_columns(line);
        let app_id = cols.first().copied().unwrap_or("").trim();
        // Valid flatpak app IDs always contain a dot (e.g. "org.mozilla.firefox");
        // this naturally filters out any header row.
        if app_id.is_empty() || !app_id.contains('.') {
            continue;
        }
        let description = cols.get(1).copied().unwrap_or("").trim().to_string();
        let installed = installed_flatpaks.contains(app_id);
        records.push(PackageRecord {
            name: app_id.to_string(),
            source: PackageSource::Flatpak,
            description,
            installed,
        });
    }
    records
}

/// Return the set of currently-installed flatpak application IDs.
fn flatpak_installed_set() -> std::collections::HashSet<String> {
    let output = Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = output else {
        return std::collections::HashSet::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Split a row into columns: prefer tab-separated, fall back to 2+ consecutive spaces.
fn split_columns(line: &str) -> Vec<&str> {
    if line.contains('\t') {
        return line.split('\t').collect();
    }
    let bytes = line.as_bytes();
    let mut cols: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
            cols.push(line[start..i].trim());
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            start = i;
        } else {
            i += 1;
        }
    }
    cols.push(line[start..].trim());
    cols
}

fn relevance_score(record: &PackageRecord, query: &str) -> u8 {
    let name_lc = record.display_name().to_lowercase();
    let q = query.to_lowercase();
    if name_lc == q {
        100
    } else if name_lc.starts_with(&q) {
        80
    } else if name_lc.contains(&q) {
        60
    } else {
        30
    }
}

/// Unified search across native manager, AUR, and flatpak; ranked by name relevance.
/// Sources are queried in parallel, so total time equals the slowest,
/// not the sum.
pub fn search_packages(query: &str, managers: &ManagerContext) -> Vec<PackageRecord> {
    let q_native = query.to_string();
    let native = managers.native.clone();
    let h_native = std::thread::spawn(move || match native {
        Some(NativeManager::Pacman) => query_pacman(&q_native),
        Some(NativeManager::Apt { .. }) => query_apt(&q_native),
        Some(NativeManager::Dnf) => query_dnf(&q_native),
        Some(NativeManager::Zypper) => query_zypper(&q_native),
        None => vec![],
    });

    let q_aur = query.to_string();
    let aur = managers.aur_helper.clone();
    let h_aur = std::thread::spawn(move || match aur {
        Some(helper) => query_aur(&q_aur, &helper),
        None => vec![],
    });

    let q_flatpak = query.to_string();
    let flatpak_enabled = managers.flatpak_available;
    let h_flatpak = std::thread::spawn(move || {
        if flatpak_enabled {
            query_flatpak(&q_flatpak)
        } else {
            vec![]
        }
    });

    let mut results = h_native.join().unwrap_or_default();
    results.extend(h_aur.join().unwrap_or_default());
    results.extend(h_flatpak.join().unwrap_or_default());

    // Deduplicate by lowercase name, keeping the first occurrence
    let mut seen = std::collections::HashSet::new();
    results.retain(|r| seen.insert(r.name.to_lowercase()));
    // Sort: highest relevance first, then shorter name first on ties
    results.sort_by(|a, b| {
        let sa = relevance_score(a, query);
        let sb = relevance_score(b, query);
        sb.cmp(&sa).then_with(|| a.name.len().cmp(&b.name.len()))
    });
    results
}

// ── Installed app discovery (for launcher) ────────────────────────────────

/// Discover installed GUI apps by reading .desktop files from standard locations.
pub fn discover_installed_apps() -> Vec<LaunchProfile> {
    let home = std::env::var("HOME").unwrap_or_default();
    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", home));
    let search_dirs = vec![
        std::path::PathBuf::from("/usr/share/applications"),
        std::path::PathBuf::from(format!("{}/applications", xdg_data_home)),
        std::path::PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        std::path::PathBuf::from(format!("{}/flatpak/exports/share/applications", xdg_data_home)),
    ];
    let mut profiles: Vec<LaunchProfile> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(profile) = parse_desktop_file(&path) {
                let key = profile.name.to_lowercase();
                if !seen.contains(&key) {
                    seen.insert(key);
                    profiles.push(profile);
                }
            }
        }
    }
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    profiles
}

fn parse_desktop_file(path: &std::path::Path) -> Option<LaunchProfile> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut working_dir = String::new();
    let mut is_application = false;
    let mut skip = false;
    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_entry = true;
            continue;
        }
        if line.starts_with('[') {
            if in_entry {
                break;
            }
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            if name.is_empty() {
                name = v.to_string();
            }
        } else if let Some(v) = line.strip_prefix("Exec=") {
            if exec.is_empty() {
                exec = v.to_string();
            }
        } else if let Some(v) = line.strip_prefix("Path=") {
            working_dir = v.to_string();
        } else if line == "Type=Application" {
            is_application = true;
        } else if line == "NoDisplay=true" || line == "Hidden=true" {
            skip = true;
        }
    }
    if !is_application || skip || name.is_empty() || exec.is_empty() {
        return None;
    }
    // Strip Exec field codes (%u %U %f %F %i %c %k etc.) and split into argv
    let argv: Vec<&str> = exec
        .split_whitespace()
        .filter(|t| !(t.len() == 2 && t.starts_with('%')))
        .collect();
    let command = argv.first()?.to_string();
    let args = argv[1..].iter().map(|s| s.to_string()).collect();
    Some(LaunchProfile {
        name,
        command,
        args,
        env: vec![],
        working_dir,
    })
}

// ── PATH command discovery (for launcher command mode) ────────────────────

/// Return a sorted, deduplicated list of executable names found in PATH.
pub fn discover_path_commands() -> Vec<String> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    let mut names: Vec<String> = Vec::new();
    for dir in path_var.split(':') {
        let dir = std::path::Path::new(dir);
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // Keep only regular files (or symlinks to them) that are executable.
            let Ok(meta) = path.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 == 0 {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if seen.insert(name.to_string()) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    names
}

// ── Update discovery ───────────────────────────────────────────────────────

/// Check for available pacman updates, returns package names.
pub fn check_pacman_updates() -> Vec<String> {
    let out = Command::new("checkupdates").output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.split_whitespace().next().map(str::to_string))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check for available apt/nala updates, returns package names.
pub fn check_apt_updates() -> Vec<String> {
    let out = Command::new("sh")
        .args(["-c", "apt list --upgradable 2>/dev/null"])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                if line.starts_with("Listing...") || line.trim().is_empty() {
                    return None;
                }
                line.split('/').next().map(str::trim).map(str::to_string)
            })
            .filter(|name| !name.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check for available dnf updates, returns package names.
pub fn check_dnf_updates() -> Vec<String> {
    let out = Command::new("dnf").args(["check-update", "--quiet"]).output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with("Last metadata") {
                    return None;
                }
                let first = line.split_whitespace().next()?;
                if !first.contains('.') {
                    return None;
                }
                Some(first.split('.').next().unwrap_or(first).to_string())
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check for available zypper updates, returns package names.
pub fn check_zypper_updates() -> Vec<String> {
    let out = Command::new("zypper").args(["list-updates"]).output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                if !line.contains('|') || line.starts_with('-') {
                    return None;
                }
                let cols: Vec<&str> = line.split('|').map(|c| c.trim()).collect();
                if cols.len() < 4 {
                    return None;
                }
                let name = cols[3];
                if name.is_empty() || name.eq_ignore_ascii_case("Name") {
                    return None;
                }
                Some(name.to_string())
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check for flatpak updates, returns app IDs.
pub fn check_flatpak_updates() -> Vec<String> {
    let out = Command::new("flatpak")
        .args(["remote-ls", "--updates", "--columns=application"])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .skip(1)
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_upgrade_order_is_stable() {
        let ctx = ManagerContext {
            native: Some(NativeManager::Pacman),
            aur_helper: Some("paru".to_string()),
            flatpak_available: true,
        };
        let cmds = full_upgrade_commands(&ctx);
        assert_eq!(cmds[0], "sudo pacman -Syu --noconfirm");
        assert_eq!(cmds[1], "paru -Sua --noconfirm");
        assert_eq!(cmds[2], "flatpak update -y");
    }

    #[test]
    fn dry_run_never_executes() {
        let r = run_command("exit 1", true).unwrap();
        assert!(r.dry_run);
        assert!(r.success);
    }

    #[test]
    fn run_sequence_stops_on_failure() {
        let cmds = vec![
            "true".to_string(),
            "false".to_string(),
            "true".to_string(),
        ];
        let results = run_sequence(&cmds, false);
        assert_eq!(results.len(), 2);
        assert!(!results[1].success);
    }

    #[test]
    fn parse_pacman_output_parses_name() {
        let raw = "extra/neovim 0.10.0\n    Hyperextensible Vim-based text editor\n";
        let recs = parse_pacman_output(raw);
        assert_eq!(recs[0].name, "neovim");
        assert_eq!(recs[0].description, "Hyperextensible Vim-based text editor");
    }
}

