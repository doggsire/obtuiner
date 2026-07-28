use std::{env, process};

use plugin_api::{handle_metadata_handshake, PluginMetadata};

fn metadata() -> PluginMetadata {
    PluginMetadata::new(
        "powermenu",
        vec!["-p".to_string()],
        "Power menu: shutdown, reboot, sleep, logout",
    )
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    // Answer the root CLI's plugin discovery handshake and exit before doing
    // anything else (e.g. touching the terminal).
    handle_metadata_handshake(&args, &metadata());

    match powermenu::run(&args) {
        Ok(()) => process::exit(0),
        Err(err) => {
            eprintln!("powermenu failed: {}", err);
            process::exit(1);
        }
    }
}
