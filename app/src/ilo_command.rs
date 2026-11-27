//! The command an agent in a terminal runs to use the browser, one tool call a run, as the agent
//! Settings › Agents › Command line added (src/agents/command.rs).

use std::io::{Read, Write};

use browser::agents::command;

fn main() {
    let name = std::env::args_os().next().unwrap_or_default();
    let printed = match command::renamed(&name) {
        Some(printed) => printed,
        None => {
            let agent = command::added(std::path::Path::new(&browser::profile_dir()));
            let named = std::env::var(command::NAME).ok();
            let input = || {
                let mut text = String::new();
                _ = std::io::stdin().read_to_string(&mut text);
                text
            };
            command::run(std::env::args().skip(1).collect(), named, agent, input)
        }
    };
    _ = std::io::stdout().write_all(printed.out.as_bytes());
    _ = std::io::stderr().write_all(printed.err.as_bytes());
    std::process::exit(printed.code);
}
