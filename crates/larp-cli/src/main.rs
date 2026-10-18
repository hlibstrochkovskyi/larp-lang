use larp_source::{SourceFile, SourceId};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "LARP — Fake it till you compile.

Usage:
  larp --help
  larp --version
  larp explain CODE
  larp build FILE.larp [-o FILE.larpbc]
  larp check FILE.larp|FILE.larpbc
  larp run FILE.larp|FILE.larpbc [--fuel INSTRUCTIONS] [--gc-stress]
  larp dump FILE.larp|FILE.larpbc [--ir]

Implemented: scalar types, first-class functions, closures, arrays, options/results,
archetypes, roles, extensions, constructors, private/public fields, bound methods,
branches, loops, checked conversions, effect bounds, typed IR,
verified bytecode and versioned artifacts, eager collection pipelines, isolated scenarios,
pure memoized lazy bindings, dynamic observation suppression, and tracing GC. palantir reports go to stderr.
Artifacts embed source maps and are verified again when loaded. --ir requires source.
Planned: fmt.
--gc-stress collects at every object/array/closure/lazy allocation safepoint.";

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
                "larp {} (objects, scenarios, and memoized lazy values)",
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
        ["build", path] => process(path, Mode::Build(Path::new(path).with_extension("larpbc"))),
        ["build", path, "-o", output] => process(path, Mode::Build(PathBuf::from(output))),
        ["run", path, options @ ..] => match run_limits(options) {
            Ok(limits) => process(path, Mode::Run(limits)),
            Err(message) => {
                eprintln!("error: {message}");
                ExitCode::from(2)
            }
        },
        ["dump", path] => process(path, Mode::Dump),
        ["dump", path, "--ir"] => process(path, Mode::Ir),
        [command @ "fmt", ..] => {
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

fn run_limits(mut options: &[&str]) -> Result<larp_vm::Limits, &'static str> {
    let mut limits = larp_vm::Limits::default();
    let mut fuel_seen = false;
    while !options.is_empty() {
        options = match options {
            ["--gc-stress", rest @ ..] if !limits.gc_stress => {
                limits.gc_stress = true;
                rest
            }
            ["--fuel", value, rest @ ..] if !fuel_seen => {
                limits.instructions = value
                    .parse()
                    .map_err(|_| "--fuel requires a nonnegative u64 instruction count")?;
                fuel_seen = true;
                rest
            }
            _ => return Err("unknown, duplicate, or incomplete run option; see larp --help"),
        };
    }
    Ok(limits)
}

enum Mode {
    Check,
    Run(larp_vm::Limits),
    Dump,
    Ir,
    Build(PathBuf),
}

fn process(path: &str, mode: Mode) -> ExitCode {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) => return input_error(path, &error),
    };
    let mut prefix = Vec::new();
    if let Err(error) = Read::by_ref(&mut file).take(8).read_to_end(&mut prefix) {
        return input_error(path, &error);
    }
    if let Err(error) = file.rewind() {
        return input_error(path, &error);
    }
    if prefix == b"LARPBC\r\n"
        || Path::new(path)
            .extension()
            .is_some_and(|ext| ext == "larpbc")
    {
        if matches!(mode, Mode::Ir | Mode::Build(_)) {
            eprintln!("error: build and dump --ir require source input");
            return ExitCode::from(2);
        }
        let artifact = match larp_artifact::read(&mut file) {
            Ok(artifact) => artifact,
            Err(error) => {
                let code = match error.kind() {
                    larp_artifact::ErrorKind::InvalidBytecode => "LARP-B001",
                    larp_artifact::ErrorKind::ResourceLimit => "LARP-B003",
                    _ => "LARP-B002",
                };
                eprintln!("error[{code}]: cannot load '{path}': {error}");
                return ExitCode::from(1);
            }
        };
        return execute(&artifact.module, &artifact.sources, mode);
    }
    let mut text = String::new();
    if let Err(error) = file.read_to_string(&mut text) {
        return input_error(path, &error);
    }
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
        Mode::Ir => {
            println!("{:#?}", compilation.ir);
            ExitCode::SUCCESS
        }
        Mode::Build(output) => {
            if output == Path::new(path)
                || std::fs::canonicalize(&output).ok().is_some_and(|output| {
                    std::fs::canonicalize(path).is_ok_and(|input| input == output)
                })
            {
                eprintln!("error: artifact output must differ from the source file");
                return ExitCode::from(2);
            }
            let bytes = match larp_artifact::encode(&compilation.bytecode, &[source]) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("error[LARP-B003]: cannot encode artifact: {error}");
                    return ExitCode::from(1);
                }
            };
            if let Err(error) = write_artifact(&output, &bytes) {
                eprintln!("error: cannot write '{}': {error}", output.display());
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        mode => execute(&compilation.bytecode, &[source], mode),
    }
}

fn input_error(path: &str, error: &std::io::Error) -> ExitCode {
    eprintln!("error: cannot read '{path}': {error}");
    ExitCode::from(1)
}

fn execute(module: &larp_bytecode::VerifiedModule, sources: &[SourceFile], mode: Mode) -> ExitCode {
    match mode {
        Mode::Check => {}
        Mode::Dump => print!("{}", module.disassemble()),
        Mode::Run(limits) => {
            if let Err(diagnostic) = larp_vm::execute_with_observers(
                module,
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
                limits,
            ) {
                if let Some(source) = sources
                    .iter()
                    .find(|source| source.slice(diagnostic.primary.span).is_some())
                {
                    eprint!("{}", larp_diagnostics::render(&diagnostic, source));
                } else {
                    eprintln!("error[{}]: {}", diagnostic.code.id, diagnostic.message);
                }
                return ExitCode::from(1);
            }
        }
        Mode::Build(_) | Mode::Ir => {
            unreachable!("source-only commands are handled before execution")
        }
    }
    ExitCode::SUCCESS
}

/// Publish a complete file with a sibling rename. Failed builds keep prior output.
fn write_artifact(output: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = output
        .file_name()
        .ok_or_else(|| std::io::Error::other("output needs a file name"))?;
    let mut temporary = None;
    for _ in 0..64 {
        let mut temp_name = std::ffi::OsString::from(".");
        temp_name.push(name);
        temp_name.push(format!(
            ".{}-{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let path = parent.join(temp_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                temporary = Some((path, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    let (path, mut file) =
        temporary.ok_or_else(|| std::io::Error::other("cannot reserve temporary output"))?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&path, output)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}
