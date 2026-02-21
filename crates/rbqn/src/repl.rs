use crate::bootstrap::Runtime;

pub fn run_repl(_rt: &Runtime) {
    eprintln!("rbqn: REPL not yet available (requires rustyline)");
    eprintln!("rbqn: use -e or -p to evaluate expressions");
}
