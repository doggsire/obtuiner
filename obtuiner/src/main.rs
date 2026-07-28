use std::env;
use std::ffi::OsString;
use std::process;

use anyhow::Result;

mod plugins;

use plugins::{discover_plugins, ExternalPlugin};

/// A built-in tool's canonical name and its short flag aliases.
struct BuiltinSpec {
    name: &'static str,
    aliases: &'static [&'static str],
}

const BUILTINS: &[BuiltinSpec] = &[
    BuiltinSpec {
        name: "installer",
        aliases: &["-i"],
    },
    BuiltinSpec {
        name: "launcher",
        aliases: &["-l"],
    },
    BuiltinSpec {
        name: "updater",
        aliases: &["-u"],
    },
];

enum ResolvedTool {
    Builtin(&'static str),
    Plugin(ExternalPlugin),
}

fn usage(program: &str) {
    eprintln!(
        "Usage: {} <installer|launcher|updater|-i|-l|-u> [args...]",
        program
    );

    let plugins = discover_plugins();
    if !plugins.is_empty() {
        eprintln!();
        eprintln!("Discovered plugins:");
        for plugin in &plugins {
            let aliases = if plugin.metadata.aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", plugin.metadata.aliases.join(", "))
            };
            eprintln!(
                "  {}{} - {}",
                plugin.metadata.name, aliases, plugin.metadata.summary
            );
        }
    }
}

/// Resolve a built-in tool name/alias without touching the filesystem.
fn resolve_builtin(tool: &str) -> Option<&'static str> {
    BUILTINS
        .iter()
        .find(|spec| spec.name == tool || spec.aliases.contains(&tool))
        .map(|spec| spec.name)
}

/// Resolve `tool` to a built-in or a discovered external plugin. Built-ins
/// are checked first and require no I/O; plugin discovery only runs when the
/// tool isn't a recognized built-in.
fn resolve_tool(tool: &str) -> Option<ResolvedTool> {
    if let Some(name) = resolve_builtin(tool) {
        return Some(ResolvedTool::Builtin(name));
    }
    discover_plugins()
        .into_iter()
        .find(|plugin| plugin.metadata.matches(tool))
        .map(ResolvedTool::Plugin)
}

fn run_builtin(tool: &str, tool_args: &[String]) -> Result<()> {
    match tool {
        "installer" => installer::run(tool_args),
        "launcher" => launcher::run(tool_args),
        "updater" => updater::run(tool_args),
        _ => unreachable!("tool should be validated before dispatch"),
    }
}

fn main() {
    let mut args = env::args_os();
    let program = args
        .next()
        .unwrap_or_else(|| OsString::from("obtuiner"))
        .to_string_lossy()
        .into_owned();

    let Some(tool) = args.next() else {
        usage(&program);
        process::exit(2);
    };

    let tool_str = tool.to_string_lossy().into_owned();
    let passthrough: Vec<String> = args.map(|a| a.to_string_lossy().into_owned()).collect();

    let Some(resolved) = resolve_tool(&tool_str) else {
        usage(&program);
        eprintln!("Unknown tool: {}", tool_str);
        process::exit(2);
    };

    let result = match &resolved {
        ResolvedTool::Builtin(name) => run_builtin(name, &passthrough),
        ResolvedTool::Plugin(plugin) => plugin.run(&passthrough),
    };

    match result {
        Ok(()) => process::exit(0),
        Err(err) => {
            eprintln!("{} failed: {}", tool_str, err);
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_builtin;

    #[test]
    fn resolves_full_tool_names() {
        assert_eq!(resolve_builtin("installer"), Some("installer"));
        assert_eq!(resolve_builtin("launcher"), Some("launcher"));
        assert_eq!(resolve_builtin("updater"), Some("updater"));
    }

    #[test]
    fn resolves_short_flags() {
        assert_eq!(resolve_builtin("-i"), Some("installer"));
        assert_eq!(resolve_builtin("-l"), Some("launcher"));
        assert_eq!(resolve_builtin("-u"), Some("updater"));
    }

    #[test]
    fn rejects_unknown_tool() {
        assert_eq!(resolve_builtin("-x"), None);
        assert_eq!(resolve_builtin("foo"), None);
    }
}
