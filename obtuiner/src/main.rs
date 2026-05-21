use std::env;
use std::ffi::OsString;
use std::process;

use anyhow::Result;

fn usage(program: &str) {
    eprintln!(
        "Usage: {} <installer|launcher|updater|-i|-l|-u> [args...]",
        program
    );
}

fn resolve_tool_name(tool: &str) -> Option<&'static str> {
    match tool {
        "installer" | "-i" => Some("installer"),
        "launcher" | "-l" => Some("launcher"),
        "updater" | "-u" => Some("updater"),
        _ => None,
    }
}

fn run_tool(tool: &str, tool_args: &[String]) -> Result<()> {
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

    let tool_str = tool.to_string_lossy();
    let Some(tool_name) = resolve_tool_name(tool_str.as_ref()) else {
        usage(&program);
        eprintln!("Unknown tool: {}", tool_str);
        process::exit(2);
    };

    let passthrough: Vec<String> = args.map(|a| a.to_string_lossy().into_owned()).collect();
    match run_tool(tool_name, &passthrough) {
        Ok(()) => process::exit(0),
        Err(err) => {
            eprintln!("{} failed: {}", tool_name, err);
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_tool_name;

    #[test]
    fn resolves_full_tool_names() {
        assert_eq!(resolve_tool_name("installer"), Some("installer"));
        assert_eq!(resolve_tool_name("launcher"), Some("launcher"));
        assert_eq!(resolve_tool_name("updater"), Some("updater"));
    }

    #[test]
    fn resolves_short_flags() {
        assert_eq!(resolve_tool_name("-i"), Some("installer"));
        assert_eq!(resolve_tool_name("-l"), Some("launcher"));
        assert_eq!(resolve_tool_name("-u"), Some("updater"));
    }

    #[test]
    fn rejects_unknown_tool() {
        assert_eq!(resolve_tool_name("-x"), None);
        assert_eq!(resolve_tool_name("foo"), None);
    }
}
