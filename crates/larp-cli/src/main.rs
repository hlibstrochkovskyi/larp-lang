use std::process::ExitCode;

const HELP: &str = "LARP — Fake it till you compile.

Usage:
  larp --help
  larp --version
  larp explain CODE

Implemented: diagnostic explanations and project infrastructure.
Planned: check, build, run, fmt. No LARP compiler or VM is implemented yet.";

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let Some(args) = args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<Vec<_>>>()
    else {
        eprintln!("error: CLI arguments must be valid UTF-8");
        return ExitCode::from(2);
    };
    match args.as_slice() {
        [] | ["--help"] | ["-h"] => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        ["--version"] | ["-V"] => {
            println!(
                "larp {} (foundation; compiler unavailable)",
                env!("CARGO_PKG_VERSION")
            );
            ExitCode::SUCCESS
        }
        ["explain", id] => match larp_diagnostics::lookup(id) {
            Some(code) => {
                println!("{} — {}\n\n{}", code.id, code.title, code.explanation);
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("error: unknown diagnostic code {id}");
                ExitCode::from(2)
            }
        },
        [command @ ("check" | "build" | "run" | "fmt"), ..] => {
            eprintln!(
                "error: '{command}' is not implemented; this repository is at the design/foundation stage"
            );
            ExitCode::from(2)
        }
        _ => {
            eprintln!("error: invalid arguments\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}
