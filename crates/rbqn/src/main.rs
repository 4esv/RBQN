mod bootstrap;
mod cli;
mod embedded;
mod repl;

use rbqn_core::error::BqnError;
use rbqn_prim::dispatch::PrimitiveRegistry;

fn main() {
    let args = cli::parse_args();

    if let cli::Mode::Help = args.mode {
        cli::print_help();
        return;
    }

    let prims = PrimitiveRegistry::new();
    let rt = match bootstrap::bootstrap(&prims) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("rbqn: bootstrap failed: {e}");
            std::process::exit(1);
        }
    };

    let _ = args.heap_max; // TODO: enforce heap limit

    let result = match args.mode {
        cli::Mode::Help => unreachable!(),
        cli::Mode::Repl => {
            repl::run_repl(&rt);
            Ok(())
        }
        cli::Mode::Eval(code) => exec_string(&rt, &code).map(|_| ()),
        cli::Mode::Print(code) => {
            exec_string(&rt, &code).map(|r| {
                println!("{}", format_result(&rt, &r));
            })
        }
        cli::Mode::Output(code) => {
            exec_string(&rt, &code).map(|r| {
                print!("{}", format_raw(&r));
            })
        }
        cli::Mode::File(path, _file_args) => {
            if path == "-" {
                let code = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
                exec_string(&rt, &code).map(|_| ())
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(code) => exec_string(&rt, &code).map(|_| ()),
                    Err(e) => Err(BqnError::Internal(format!("cannot read {path}: {e}"))),
                }
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn exec_string(
    _rt: &bootstrap::Runtime,
    _code: &str,
) -> rbqn_core::error::Result<rbqn_core::value::B> {
    // TODO: compile and execute code using rt.compiler
    Err(BqnError::Nyi(
        "expression evaluation not yet implemented".into(),
    ))
}

fn format_result(rt: &bootstrap::Runtime, val: &rbqn_core::value::B) -> String {
    if let Some((ref fmt, _)) = rt.formatter {
        let _ = fmt; // TODO: use formatter
    }
    format!("{val}")
}

fn format_raw(val: &rbqn_core::value::B) -> String {
    match val {
        rbqn_core::value::B::Arr(items, _) => {
            items
                .iter()
                .filter_map(|b| b.as_char())
                .collect()
        }
        other => format!("{other}"),
    }
}
