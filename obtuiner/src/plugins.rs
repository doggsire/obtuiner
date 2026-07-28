//! Discovery of external plugin executables for the root CLI.
//!
//! A plugin is any executable found on `PATH` or in
//! [`core_domain::plugins_dir`] whose file name starts with
//! [`plugin_api::PLUGIN_EXECUTABLE_PREFIX`]. Each candidate is invoked with
//! [`plugin_api::METADATA_FLAG`]; only executables that respond with a valid
//! [`plugin_api::PluginMetadata`] JSON document are registered.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use plugin_api::{PluginMetadata, METADATA_FLAG, PLUGIN_EXECUTABLE_PREFIX};

/// A discovered plugin executable, ready to be dispatched to.
#[derive(Debug, Clone)]
pub struct ExternalPlugin {
    pub metadata: PluginMetadata,
    pub executable_path: PathBuf,
}

impl ExternalPlugin {
    /// Run the plugin executable with `args`, inheriting the current
    /// process's stdio so it can drive its own terminal UI.
    pub fn run(&self, args: &[String]) -> Result<()> {
        let status = Command::new(&self.executable_path)
            .args(args)
            .status()
            .with_context(|| {
                format!(
                    "failed to launch plugin '{}'",
                    self.executable_path.display()
                )
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "plugin '{}' exited with status {}",
                self.metadata.name,
                status
            ))
        }
    }
}

/// Directories to search for plugin executables, in priority order.
fn plugin_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![core_domain::plugins_dir()];
    if let Some(path_var) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path_var));
    }
    dirs
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn query_metadata(path: &Path) -> Option<PluginMetadata> {
    let output = Command::new(path).arg(METADATA_FLAG).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    PluginMetadata::from_json(text.trim()).ok()
}

/// Scan configured plugin directories and `PATH` for executables named
/// `obtuiner-*`, query each for metadata, and return the ones that respond
/// with a valid manifest. The first match found for a given canonical name
/// wins if multiple candidates report the same name.
pub fn discover_plugins() -> Vec<ExternalPlugin> {
    let mut seen_names = HashSet::new();
    let mut plugins = Vec::new();

    for dir in plugin_search_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !file_name.starts_with(PLUGIN_EXECUTABLE_PREFIX) {
                continue;
            }
            if !is_executable(&path) {
                continue;
            }
            let Some(metadata) = query_metadata(&path) else {
                continue;
            };
            if !seen_names.insert(metadata.name.clone()) {
                continue;
            }
            plugins.push(ExternalPlugin {
                metadata,
                executable_path: path,
            });
        }
    }

    plugins
}
