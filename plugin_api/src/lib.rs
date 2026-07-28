//! Shared contract between the `obtuiner` root CLI and external plugin
//! executables (e.g. `obtuiner-powermenu`).
//!
//! A plugin is any executable on `PATH` (or in Obtuiner's plugin directory)
//! whose file name starts with [`PLUGIN_EXECUTABLE_PREFIX`]. The root CLI
//! discovers plugins by invoking each candidate with [`METADATA_FLAG`]; a
//! valid plugin responds by printing a single-line JSON [`PluginMetadata`]
//! document to stdout and exiting successfully. See
//! [`handle_metadata_handshake`] for the plugin-side helper.

use serde::{Deserialize, Serialize};

/// CLI flag a plugin executable must respond to with its metadata JSON on
/// stdout, then exit successfully.
pub const METADATA_FLAG: &str = "--obtuiner-plugin-metadata";

/// Required file name prefix for a plugin executable to be considered during
/// discovery (e.g. `obtuiner-powermenu`).
pub const PLUGIN_EXECUTABLE_PREFIX: &str = "obtuiner-";

/// Metadata a plugin reports to the root CLI so it can be resolved by name
/// or alias and listed in help output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Canonical tool name, e.g. `"powermenu"`.
    pub name: String,
    /// Short flag aliases, e.g. `["-p"]`.
    pub aliases: Vec<String>,
    /// One-line human-readable description shown in usage output.
    pub summary: String,
}

impl PluginMetadata {
    pub fn new(name: impl Into<String>, aliases: Vec<String>, summary: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            aliases,
            summary: summary.into(),
        }
    }

    /// True if `tool` matches this plugin's canonical name or one of its
    /// aliases exactly.
    pub fn matches(&self, tool: &str) -> bool {
        self.name == tool || self.aliases.iter().any(|alias| alias == tool)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Plugin-side helper: call at the very top of `main()`, before parsing any
/// real arguments. If `args` contains [`METADATA_FLAG`], this prints the
/// plugin's metadata as JSON and exits the process immediately; otherwise it
/// returns and normal argument handling should continue.
pub fn handle_metadata_handshake(args: &[String], metadata: &PluginMetadata) {
    if args.iter().any(|arg| arg == METADATA_FLAG) {
        match metadata.to_json() {
            Ok(json) => println!("{}", json),
            Err(err) => eprintln!("failed to serialize plugin metadata: {}", err),
        }
        std::process::exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_canonical_name() {
        let meta = PluginMetadata::new("powermenu", vec!["-p".to_string()], "Power menu");
        assert!(meta.matches("powermenu"));
    }

    #[test]
    fn matches_alias() {
        let meta = PluginMetadata::new("powermenu", vec!["-p".to_string()], "Power menu");
        assert!(meta.matches("-p"));
        assert!(!meta.matches("-x"));
    }

    #[test]
    fn json_round_trips() {
        let meta = PluginMetadata::new("powermenu", vec!["-p".to_string()], "Power menu");
        let json = meta.to_json().unwrap();
        let back = PluginMetadata::from_json(&json).unwrap();
        assert_eq!(meta, back);
    }
}
