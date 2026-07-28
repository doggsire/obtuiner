use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ── Domain models ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageSource {
    Pacman,
    Aur,
    Apt,
    Dnf,
    Zypper,
    Flatpak,
}

impl PackageSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pacman => "pacman",
            Self::Aur => "aur",
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Zypper => "zypper",
            Self::Flatpak => "flatpak",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageRecord {
    pub name: String,
    pub source: PackageSource,
    pub description: String,
    #[serde(default)]
    pub installed: bool,
}

impl PackageRecord {
    /// User-visible name: for Flatpak, strips the reverse-domain prefix
    /// (e.g. "org.vinegarhq.Sober" → "Sober"); for others returns name as-is.
    pub fn display_name(&self) -> &str {
        match self.source {
            PackageSource::Flatpak => self.name.rsplit('.').next().unwrap_or(&self.name),
            _ => &self.name,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchProfile {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub working_dir: String,
}

#[derive(Clone, Debug)]
pub enum UpdaterTarget {
    FullSystem,
    Package(PackageRecord),
}

// ── XDG path helpers ───────────────────────────────────────────────────────

const APP_QUALIFIER: &str = "ui";

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "{}/.config",
                std::env::var("HOME").unwrap_or_default()
            ))
        })
        .join(APP_QUALIFIER)
}

pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "{}/.local/share",
                std::env::var("HOME").unwrap_or_default()
            ))
        })
        .join(APP_QUALIFIER)
}

/// Directory scanned by the root CLI for external plugin executables, in
/// addition to `PATH`.
pub fn plugins_dir() -> PathBuf {
    data_dir().join("plugins")
}

// ── Launcher profile persistence ───────────────────────────────────────────

fn profiles_path() -> PathBuf {
    config_dir().join("launcher").join("profiles.json")
}

pub fn load_profiles() -> Result<Vec<LaunchProfile>> {
    let path = profiles_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&contents)?)
}

pub fn save_profiles(profiles: &[LaunchProfile]) -> Result<()> {
    let path = profiles_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(profiles)?;
    std::fs::write(&path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_source_serializes() {
        let json = serde_json::to_string(&PackageSource::Aur).unwrap();
        assert_eq!(json, "\"aur\"");
    }

    #[test]
    fn profile_round_trips_json() {
        let profile = LaunchProfile {
            name: "test".into(),
            command: "echo".into(),
            args: vec!["hello".into()],
            env: vec![("KEY".into(), "val".into())],
            working_dir: "/tmp".into(),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let back: LaunchProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, profile.name);
    }
}
