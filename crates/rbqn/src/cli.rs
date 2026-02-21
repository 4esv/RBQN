use std::process;

pub struct Args {
    pub mode: Mode,
    pub heap_max: Option<u64>,
}

pub enum Mode {
    Repl,
    Eval(String),
    Print(String),
    Output(String),
    File(String, Vec<String>),
    Help,
}

pub fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let program = &args[0];

    if args.len() == 1 {
        return Args { mode: Mode::Repl, heap_max: None };
    }

    let mut heap_max = None;
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];

        if !arg.starts_with('-') {
            // Positional argument — treat as file
            let file = arg.clone();
            let file_args: Vec<String> = args[i + 1..].to_vec();
            return Args { mode: Mode::File(file, file_args), heap_max };
        }

        if arg == "-" {
            // Read from stdin
            let file_args: Vec<String> = args[i + 1..].to_vec();
            return Args { mode: Mode::File("-".into(), file_args), heap_max };
        }

        if arg == "--help" {
            return Args { mode: Mode::Help, heap_max: None };
        }

        if arg == "--version" {
            println!("rbqn {}", env!("CARGO_PKG_VERSION"));
            process::exit(0);
        }

        // Parse single-character flags
        let chars: Vec<char> = arg[1..].chars().collect();
        let mut j = 0;
        while j < chars.len() {
            match chars[j] {
                'h' => {
                    return Args { mode: Mode::Help, heap_max: None };
                }
                'r' => {
                    return Args { mode: Mode::Repl, heap_max };
                }
                'e' => {
                    if j + 1 < chars.len() {
                        eprintln!("{program}: -e must end the option");
                        process::exit(1);
                    }
                    i += 1;
                    if i >= args.len() {
                        eprintln!("{program}: -e requires an argument");
                        process::exit(1);
                    }
                    return Args { mode: Mode::Eval(args[i].clone()), heap_max };
                }
                'p' => {
                    if j + 1 < chars.len() {
                        eprintln!("{program}: -p must end the option");
                        process::exit(1);
                    }
                    i += 1;
                    if i >= args.len() {
                        eprintln!("{program}: -p requires an argument");
                        process::exit(1);
                    }
                    return Args { mode: Mode::Print(args[i].clone()), heap_max };
                }
                'o' => {
                    if j + 1 < chars.len() {
                        eprintln!("{program}: -o must end the option");
                        process::exit(1);
                    }
                    i += 1;
                    if i >= args.len() {
                        eprintln!("{program}: -o requires an argument");
                        process::exit(1);
                    }
                    return Args { mode: Mode::Output(args[i].clone()), heap_max };
                }
                'f' => {
                    if j + 1 < chars.len() {
                        eprintln!("{program}: -f must end the option");
                        process::exit(1);
                    }
                    i += 1;
                    if i >= args.len() {
                        eprintln!("{program}: -f requires an argument");
                        process::exit(1);
                    }
                    let file = args[i].clone();
                    let file_args: Vec<String> = args[i + 1..].to_vec();
                    return Args { mode: Mode::File(file, file_args), heap_max };
                }
                'M' => {
                    if j + 1 < chars.len() {
                        eprintln!("{program}: -M must end the option");
                        process::exit(1);
                    }
                    i += 1;
                    if i >= args.len() {
                        eprintln!("{program}: -M requires an argument");
                        process::exit(1);
                    }
                    match args[i].parse::<u64>() {
                        Ok(mb) => heap_max = Some(mb * 1024 * 1024),
                        Err(_) => {
                            eprintln!("{program}: -M: argument not a number");
                            process::exit(1);
                        }
                    }
                }
                c => {
                    eprintln!("{program}: unknown option: -{c}");
                    process::exit(1);
                }
            }
            j += 1;
        }
        i += 1;
    }

    Args { mode: Mode::Repl, heap_max }
}

pub fn print_help() {
    let name = std::env::args().next().unwrap_or_else(|| "rbqn".into());
    println!(
        "Usage: {name} [options] [file.bqn [arguments]]\n\
         Options:\n\
         \x20 -f file    execute the contents of the file with all further arguments as *args\n\
         \x20 -e code    execute the argument as BQN\n\
         \x20 -p code    execute the argument as BQN and print its result pretty-printed\n\
         \x20 -o code    execute the argument as BQN and print its raw result\n\
         \x20 -M num     set maximum heap size to num megabytes\n\
         \x20 -r         start the REPL\n\
         \x20 --help     show this help text\n\
         \x20 --version  display version information"
    );
}
