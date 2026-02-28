use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use rbqn::bootstrap::Runtime;
use crate::ReplState;

pub fn run_repl(rt: &Runtime, silent: bool) {
    let mut rl = match DefaultEditor::new() {
        Ok(rl) => rl,
        Err(e) => {
            eprintln!("rbqn: failed to initialize REPL: {e}");
            return;
        }
    };

    // Load history
    let history_path = history_file();
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    let mut state = ReplState::new();

    loop {
        match rl.readline("   ") {
            Ok(line) => {
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);
                eval_line(rt, &line, silent, &mut state);
            }
            Err(ReadlineError::Interrupted) => {
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(e) => {
                eprintln!("rbqn: readline error: {e}");
                break;
            }
        }
    }

    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }
}

fn eval_line(rt: &Runtime, line: &str, silent: bool, state: &mut ReplState) {
    // Handle REPL commands
    if line.starts_with(')') {
        let cmd = line[1..].trim();
        if cmd == "exit" || cmd == "off" {
            std::process::exit(0);
        }
        eprintln!("Unknown REPL command: {line}");
        return;
    }

    match crate::exec_repl_line(rt, line, state) {
        Ok(result) => {
            if !silent {
                let formatted = crate::format_result(rt, &result);
                println!("{formatted}");
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    }
}

fn history_file() -> Option<String> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return Some(format!("{xdg}/.rbqn_repl_history"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(format!("{home}/.rbqn_repl_history"));
    }
    None
}
