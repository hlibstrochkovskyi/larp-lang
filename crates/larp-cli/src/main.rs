use larp_source::{SourceFile, SourceId};
use std::process::ExitCode;

const HELP: &str = "LARP — Fake it till you compile.

Usage:
  larp --help
  larp --version
  larp explain CODE
  larp check FILE.larp
  larp run FILE.larp [--fuel INSTRUCTIONS]
  larp dump FILE.larp [--ir]

Implemented: scalar types, functions, branches, loops, checked arithmetic,
effect bounds, typed IR, verified in-memory bytecode, and a stack VM.
Planned: collections, closures, objects, scenarios, persisted build artifacts, fmt.";

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
                "larp {} (executable scalar core)",
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
        ["check", path] => process(path, Mode::Check),
        ["run", path] => process(path, Mode::Run(larp_vm::Limits::default())),
        ["run", path, "--fuel", fuel] => match fuel.parse::<u64>() {
            Ok(instructions) => process(
                path,
                Mode::Run(larp_vm::Limits {
                    instructions,
                    ..Default::default()
                }),
            ),
            Err(_) => {
                eprintln!("error: --fuel requires a nonnegative u64 instruction count");
                ExitCode::from(2)
            }
        },
        ["dump", path] => process(path, Mode::Dump),
        ["dump", path, "--ir"] => process(path, Mode::Ir),
        [command @ ("build" | "fmt"), ..] => {
            eprintln!(
                "error: '{command}' is not implemented yet; see docs/implementation-status.md"
            );
            ExitCode::from(2)
        }
        _ => {
            eprintln!("error: invalid arguments\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

enum Mode {
    Check,
    Run(larp_vm::Limits),
    Dump,
    Ir,
}

fn process(path: &str, mode: Mode) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("error: cannot read '{path}' as UTF-8 source: {error}");
            return ExitCode::from(1);
        }
    };
    let source = SourceFile::new(SourceId(0), path, text);
    let compilation = match larp_compiler::compile(&source) {
        Ok(compilation) => compilation,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprint!("{}", larp_diagnostics::render(&diagnostic, &source));
            }
            return ExitCode::from(1);
        }
    };
    match mode {
        Mode::Check => {}
        Mode::Dump => print!("{}", compilation.bytecode.disassemble()),
        Mode::Ir => println!("{:#?}", compilation.ir),
        Mode::Run(limits) => {
            if let Err(diagnostic) =
                larp_vm::execute(&compilation.bytecode, &mut std::io::stdout().lock(), limits)
            {
                eprint!("{}", larp_diagnostics::render(&diagnostic, &source));
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
}
