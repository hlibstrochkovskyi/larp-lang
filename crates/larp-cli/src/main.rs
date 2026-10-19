use larp_source::{SourceFile, SourceId};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "LARP — Fake it till you compile.

Usage:
  larp [--diagnostic-format=json|text] COMMAND ...
  larp --help
  larp --version
  larp explain CODE
  larp build FILE.larp [-o FILE.larpbc]
  larp check FILE.larp|FILE.larpbc
  larp run FILE.larp|FILE.larpbc [--fuel INSTRUCTIONS] [--gc-stress] [-- ARG...]
  larp dump FILE.larp|FILE.larpbc [--ir]
  larp fmt FILE.larp [--check|--stdout]

Implemented: scalar types, first-class functions, closures, arrays, options/results,
archetypes, roles, extensions, constructors, private/public fields, bound methods,
branches, loops, checked conversions, effect bounds, typed IR,
verified bytecode and versioned artifacts, eager collection pipelines, isolated scenarios,
pure memoized lazy bindings, host arguments/stdin, dynamic observation suppression,
and tracing GC. palantir reports go to stderr.
Artifacts embed source maps and are verified again when loaded. --ir requires source.
fmt preserves tokens and comments; its default mode updates the source file.
--gc-stress collects at every object/array/closure/lazy allocation safepoint.";

#[derive(Clone, Copy)]
enum DiagnosticFormat {
    Text,
    Json,
}

impl DiagnosticFormat {
    fn event(self, kind: &str, code: Option<&str>, message: &str) {
        match self {
            Self::Text => eprintln!("{message}"),
            Self::Json => eprint!("{}", larp_diagnostics::json::event(kind, code, message)),
        }
    }

    fn error(self, message: impl std::fmt::Display) {
        self.event("cli_error", None, &format!("error: {message}"));
    }

    fn diagnostic(self, diagnostic: &larp_diagnostics::Diagnostic, sources: &[SourceFile]) {
        match self {
            Self::Json => eprint!("{}", larp_diagnostics::json::render(diagnostic, sources)),
            Self::Text => {
                if let Some(source) = sources
                    .iter()
                    .find(|source| source.slice(diagnostic.primary.span).is_some())
                {
                    eprint!("{}", larp_diagnostics::render(diagnostic, source));
                } else {
                    eprintln!("error[{}]: {}", diagnostic.code.id, diagnostic.message);
                }
            }
        }
    }
}

struct Inspection<'a> {
    output: &'a mut dyn Write,
    format: DiagnosticFormat,
}

impl Write for Inspection<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self.format {
            DiagnosticFormat::Text => self.output.write(bytes),
            DiagnosticFormat::Json => {
                let text = std::str::from_utf8(bytes)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
                self.output.write_all(
                    larp_diagnostics::json::event("inspection", None, text).as_bytes(),
                )?;
                Ok(bytes.len())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (format, args) = match args.split_first() {
        Some((flag, rest)) if flag == "--diagnostic-format=json" => (DiagnosticFormat::Json, rest),
        Some((flag, rest)) if flag == "--diagnostic-format=text" => (DiagnosticFormat::Text, rest),
        _ => (DiagnosticFormat::Text, args.as_slice()),
    };
    let Some(args) = args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<Vec<_>>>()
    else {
        format.error("CLI arguments must be valid UTF-8");
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
                format.error(format_args!("unknown diagnostic code {id}"));
                ExitCode::from(2)
            }
        },
        ["check", path] => process(path, Mode::Check, format),
        ["build", path] => process(
            path,
            Mode::Build(Path::new(path).with_extension("larpbc")),
            format,
        ),
        ["build", path, "-o", output] => process(path, Mode::Build(PathBuf::from(output)), format),
        ["run", path, options @ ..] => match run_options(options) {
            Ok(run) => process(path, Mode::Run(run), format),
            Err(message) => {
                format.error(message);
                ExitCode::from(2)
            }
        },
        ["dump", path] => process(path, Mode::Dump, format),
        ["dump", path, "--ir"] => process(path, Mode::Ir, format),
        ["fmt", path] if !path.starts_with("--") => {
            process(path, Mode::Format(FormatMode::Write), format)
        }
        ["fmt", path, "--check"] | ["fmt", "--check", path] if !path.starts_with("--") => {
            process(path, Mode::Format(FormatMode::Check), format)
        }
        ["fmt", path, "--stdout"] | ["fmt", "--stdout", path] if !path.starts_with("--") => {
            process(path, Mode::Format(FormatMode::Stdout), format)
        }
        _ => {
            format.error(format_args!("invalid arguments\n\n{HELP}"));
            ExitCode::from(2)
        }
    }
}

struct Run {
    limits: larp_vm::Limits,
    arguments: Vec<String>,
}

fn run_options(mut options: &[&str]) -> Result<Run, &'static str> {
    let mut limits = larp_vm::Limits::default();
    let mut fuel_seen = false;
    while !options.is_empty() {
        options = match options {
            ["--", arguments @ ..] => {
                return Ok(Run {
                    limits,
                    arguments: arguments.iter().map(|arg| (*arg).to_owned()).collect(),
                });
            }
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
    Ok(Run {
        limits,
        arguments: Vec::new(),
    })
}

enum Mode {
    Check,
    Run(Run),
    Dump,
    Ir,
    Build(PathBuf),
    Format(FormatMode),
}

enum FormatMode {
    Write,
    Check,
    Stdout,
}

fn process(path: &str, mode: Mode, format: DiagnosticFormat) -> ExitCode {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) => return input_error(path, &error, format),
    };
    let mut prefix = Vec::new();
    if let Err(error) = Read::by_ref(&mut file).take(8).read_to_end(&mut prefix) {
        return input_error(path, &error, format);
    }
    if let Err(error) = file.rewind() {
        return input_error(path, &error, format);
    }
    if prefix == b"LARPBC\r\n"
        || Path::new(path)
            .extension()
            .is_some_and(|ext| ext == "larpbc")
    {
        if matches!(mode, Mode::Ir | Mode::Build(_) | Mode::Format(_)) {
            format.error("build, fmt, and dump --ir require source input");
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
                format.event(
                    "artifact_error",
                    Some(code),
                    &format!("error[{code}]: cannot load '{path}': {error}"),
                );
                return ExitCode::from(1);
            }
        };
        return execute(&artifact.module, &artifact.sources, mode, format);
    }
    let text = if matches!(mode, Mode::Format(_)) {
        let mut bytes = Vec::new();
        let limit = larp_format::MAX_SOURCE_BYTES;
        if let Err(error) = file.take((limit + 1) as u64).read_to_end(&mut bytes) {
            return input_error(path, &error, format);
        }
        if bytes.len() > limit {
            let source = SourceFile::new(SourceId(0), path, "");
            let diagnostic = larp_diagnostics::Diagnostic::error(
                "LARP-P003",
                source.span(0..0).unwrap(),
                "Formatter source size limit exceeded",
            );
            format.diagnostic(&diagnostic, &[source]);
            return ExitCode::from(1);
        }
        match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                return input_error(
                    path,
                    &std::io::Error::new(std::io::ErrorKind::InvalidData, error),
                    format,
                );
            }
        }
    } else {
        let mut text = String::new();
        if let Err(error) = file.read_to_string(&mut text) {
            return input_error(path, &error, format);
        }
        text
    };
    let source = SourceFile::new(SourceId(0), path, text);
    if let Mode::Format(mode) = mode {
        return format_source(path, &source, mode, format);
    }
    let compilation = match larp_compiler::compile(&source) {
        Ok(compilation) => compilation,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                format.diagnostic(&diagnostic, std::slice::from_ref(&source));
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
                format.error("artifact output must differ from the source file");
                return ExitCode::from(2);
            }
            let bytes = match larp_artifact::encode(&compilation.bytecode, &[source]) {
                Ok(bytes) => bytes,
                Err(error) => {
                    format.event(
                        "artifact_error",
                        Some("LARP-B003"),
                        &format!("error[LARP-B003]: cannot encode artifact: {error}"),
                    );
                    return ExitCode::from(1);
                }
            };
            if let Err(error) = write_file(&output, &bytes, None, None) {
                format.error(format_args!("cannot write '{}': {error}", output.display()));
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        mode => execute(&compilation.bytecode, &[source], mode, format),
    }
}

fn input_error(path: &str, error: &std::io::Error, format: DiagnosticFormat) -> ExitCode {
    format.error(format_args!("cannot read '{path}': {error}"));
    ExitCode::from(1)
}

fn execute(
    module: &larp_bytecode::VerifiedModule,
    sources: &[SourceFile],
    mode: Mode,
    format: DiagnosticFormat,
) -> ExitCode {
    match mode {
        Mode::Check => {}
        Mode::Dump => print!("{}", module.disassemble()),
        Mode::Run(run) => {
            let mut input = std::io::stdin().lock();
            let mut host = larp_vm::Host::new(Some(&run.arguments), Some(&mut input));
            let mut stderr = std::io::stderr().lock();
            let mut inspection = Inspection {
                output: &mut stderr,
                format,
            };
            if let Err(diagnostic) = larp_vm::execute_with_host(
                module,
                &mut std::io::stdout().lock(),
                &mut inspection,
                &mut host,
                run.limits,
            ) {
                format.diagnostic(&diagnostic, sources);
                return ExitCode::from(1);
            }
        }
        Mode::Build(_) | Mode::Ir | Mode::Format(_) => {
            unreachable!("source-only commands are handled before execution")
        }
    }
    ExitCode::SUCCESS
}

fn format_source(
    path: &str,
    source: &SourceFile,
    mode: FormatMode,
    format: DiagnosticFormat,
) -> ExitCode {
    let formatted = match larp_format::format(source) {
        Ok(formatted) => formatted,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                format.diagnostic(&diagnostic, std::slice::from_ref(source));
            }
            return ExitCode::from(1);
        }
    };
    match mode {
        FormatMode::Stdout => {
            if let Err(error) = std::io::stdout().lock().write_all(formatted.as_bytes()) {
                format.error(format_args!("cannot write formatted source: {error}"));
                return ExitCode::from(1);
            }
        }
        FormatMode::Check if formatted != source.text() => {
            format.event(
                "format_needed",
                None,
                &format!("{path}: source needs formatting"),
            );
            return ExitCode::from(1);
        }
        FormatMode::Write if formatted != source.text() => {
            let result = (|| {
                // Follow source symlinks without replacing the link itself.
                let target = std::fs::canonicalize(path)?;
                let permissions = std::fs::metadata(&target)?.permissions();
                write_file(
                    &target,
                    formatted.as_bytes(),
                    Some(permissions),
                    Some(source.text().as_bytes()),
                )
            })();
            if let Err(error) = result {
                format.error(format_args!("cannot format '{path}': {error}"));
                return ExitCode::from(1);
            }
        }
        FormatMode::Check | FormatMode::Write => {}
    }
    ExitCode::SUCCESS
}

/// Publish a complete file by sibling rename. Errors preserve previous output.
fn write_file(
    output: &Path,
    bytes: &[u8],
    permissions: Option<std::fs::Permissions>,
    expected: Option<&[u8]>,
) -> std::io::Result<()> {
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
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.sync_all()?;
        drop(file);
        if let Some(expected) = expected
            && std::fs::read(output)? != expected
        {
            return Err(std::io::Error::other(
                "source changed while formatting; retry with the latest file",
            ));
        }
        std::fs::rename(&path, output)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_rejects_changed_source_and_cleans_failed_rename_output() {
        let directory =
            std::env::temp_dir().join(format!("larp-publication-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let source = directory.join("source.larp");
        std::fs::write(&source, b"latest edit").unwrap();
        let error = write_file(
            &source,
            b"stale formatted result",
            None,
            Some(b"old source"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("source changed"));
        assert_eq!(std::fs::read(&source).unwrap(), b"latest edit");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        let target_directory = directory.join("destination");
        std::fs::create_dir(&target_directory).unwrap();
        assert!(write_file(&target_directory, b"artifact", None, None).is_err());
        assert!(target_directory.is_dir());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
