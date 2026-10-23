mod manifest;
mod project;
mod scaffold;

use larp_source::{SourceFile, SourceId};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod version;

const HELP: &str = "LARP — Fake it till you compile.

Usage:
  larp [--diagnostic-format=json|text] COMMAND ...
  larp --help
  larp --version [--json]
  larp explain CODE
  larp new NAME
  larp build [FILE.larp | --manifest-path larp.toml] [-o FILE.larpbc]
  larp check [FILE.larp|FILE.larpbc | --manifest-path larp.toml]
  larp run [FILE.larp|FILE.larpbc | --manifest-path larp.toml] [--fuel INSTRUCTIONS] [--gc-stress] [-- ARG...]
  larp dump FILE.larp|FILE.larpbc [--ir]
  larp fmt FILE.larp [--check|--stdout]

Implemented: scalar types, first-class functions, closures, arrays, options/results,
archetypes, roles, extensions, constructors, private/public fields, bound methods,
branches, loops, checked conversions, effect bounds, typed IR,
verified bytecode and versioned artifacts, eager collection pipelines, isolated scenarios,
pure memoized lazy bindings, host arguments/stdin, dynamic observation suppression,
and tracing GC. palantir reports go to stderr.
With no file operand, check/run/build use the nearest ancestor larp.toml.
Project build output defaults to target/NAME.larpbc under the manifest root.
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
            Self::Text if kind == "project_error" && code.is_some() => {
                eprintln!("error[{}]: {message}", code.unwrap())
            }
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
            Self::Text => eprint!("{}", larp_diagnostics::render_sources(diagnostic, sources)),
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
        ["--version", "--json"] | ["-V", "--json"] => {
            println!("{}", version::json());
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
        ["new", name] => {
            if let Err(message) = scaffold::validate_name(name) {
                format.event("project_error", Some("LARP-J004"), &message);
                return ExitCode::from(2);
            }
            match scaffold::create(name) {
                Ok(path) => {
                    println!(
                        "Created project {}\n\n  cd {name}\n  larp check\n  larp run\n  larp build",
                        path.display()
                    );
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    format.event("project_error", Some("LARP-J004"), &message);
                    ExitCode::from(1)
                }
            }
        }
        [command @ ("check" | "build" | "run"), options @ ..] => {
            project_command(command, options, format)
        }
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

struct ProjectContext {
    project: manifest::Project,
    default_output: bool,
}

fn project_command(command: &str, mut options: &[&str], format: DiagnosticFormat) -> ExitCode {
    let mut file = None;
    let mut manifest_path = None;
    let mut output = None;
    let mut run_flags = Vec::new();
    // A positional file remains the first operand, preserving the existing CLI.
    if let Some((first, rest)) = options.split_first()
        && (!first.starts_with('-') || (!first.starts_with("--") && *first != "-o"))
    {
        file = Some(*first);
        options = rest;
    }
    let usage = || {
        format.error(
            "unknown, duplicate, conflicting, or incomplete command option; see larp --help",
        );
        ExitCode::from(2)
    };
    while !options.is_empty() {
        options = match options {
            ["--manifest-path", path, rest @ ..]
                if manifest_path.is_none()
                    && !matches!(
                        *path,
                        "--manifest-path" | "--fuel" | "--gc-stress" | "--" | "-o"
                    ) =>
            {
                manifest_path = Some(*path);
                rest
            }
            ["-o", path, rest @ ..]
                if command == "build"
                    && output.is_none()
                    && !matches!(
                        *path,
                        "--manifest-path" | "--fuel" | "--gc-stress" | "--" | "-o"
                    ) =>
            {
                output = Some(PathBuf::from(path));
                rest
            }
            ["--fuel", value, rest @ ..] if command == "run" => {
                run_flags.extend(["--fuel", *value]);
                rest
            }
            ["--gc-stress", rest @ ..] if command == "run" => {
                run_flags.push("--gc-stress");
                rest
            }
            ["--", rest @ ..] if command == "run" => {
                run_flags.push("--");
                run_flags.extend_from_slice(rest);
                &[]
            }
            _ => return usage(),
        };
    }
    if file.is_some() && manifest_path.is_some() {
        return usage();
    }
    let run = if command == "run" {
        match run_options(&run_flags) {
            Ok(run) => Some(run),
            Err(message) => {
                format.error(message);
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    let project = if file.is_none() {
        match manifest::load(manifest_path) {
            Ok(project) => Some(ProjectContext {
                project,
                default_output: command == "build" && output.is_none(),
            }),
            Err(error) => {
                match error.source {
                    Some((source, range)) => {
                        let source = *source;
                        let span = source
                            .span(range)
                            .unwrap_or_else(|| source.span(0..0).unwrap());
                        let diagnostic =
                            larp_diagnostics::Diagnostic::error(error.code, span, error.message);
                        format.diagnostic(&diagnostic, &[source]);
                    }
                    None => format.event("project_error", Some(error.code), &error.message),
                }
                return ExitCode::from(1);
            }
        }
    } else {
        None
    };
    let path = file.unwrap_or_else(|| project.as_ref().unwrap().project.entry.to_str().unwrap());
    let mode = match command {
        "check" => Mode::Check,
        "run" => Mode::Run(run.unwrap()),
        "build" => Mode::Build(output.unwrap_or_else(|| {
            match &project {
                Some(context) => context
                    .project
                    .root
                    .join("target")
                    .join(format!("{}.larpbc", context.project.name)),
                None => Path::new(path).with_extension("larpbc"),
            }
        })),
        _ => unreachable!(),
    };
    process_with_project(path, mode, format, project.as_ref())
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
    process_with_project(path, mode, format, None)
}

fn process_with_project(
    path: &str,
    mode: Mode,
    format: DiagnosticFormat,
    project_context: Option<&ProjectContext>,
) -> ExitCode {
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
    if project_context.is_none()
        && (prefix == b"LARPBC\r\n"
            || Path::new(path)
                .extension()
                .is_some_and(|ext| ext == "larpbc"))
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
    let text = if matches!(mode, Mode::Format(_)) || project_context.is_some() {
        let mut bytes = Vec::new();
        let limit = if project_context.is_some() {
            larp_compiler::project::MAX_SOURCE_BYTES
        } else {
            larp_format::MAX_SOURCE_BYTES
        };
        if let Err(error) = file.take((limit + 1) as u64).read_to_end(&mut bytes) {
            return input_error(path, &error, format);
        }
        if bytes.len() > limit {
            let source = SourceFile::new(SourceId(0), path, "");
            let diagnostic = larp_diagnostics::Diagnostic::error(
                if project_context.is_some() {
                    "LARP-M005"
                } else {
                    "LARP-P003"
                },
                source.span(0..0).unwrap(),
                if matches!(mode, Mode::Format(_)) {
                    "Formatter source size limit exceeded"
                } else {
                    "Project source size limit exceeded"
                },
            );
            format.diagnostic(&diagnostic, &[source]);
            return ExitCode::from(1);
        }
        match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                if project_context.is_some() {
                    format.event(
                        "project_error",
                        Some("LARP-J003"),
                        &format!("project entry must contain valid UTF-8: {error}"),
                    );
                    return ExitCode::from(1);
                }
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
    if project_context.is_some()
        && source.text().bytes().filter(|&byte| byte == b'\n').count() + 1
            > larp_compiler::project::MAX_SOURCE_LINES
    {
        let diagnostic = larp_diagnostics::Diagnostic::error(
            "LARP-M005",
            source.span(0..0).unwrap(),
            "Project aggregate source line limit exceeded",
        );
        format.diagnostic(&diagnostic, &[source]);
        return ExitCode::from(1);
    }
    if let Mode::Format(mode) = mode {
        return format_source(path, &source, mode, format);
    }
    // Keep the legacy grammar for existing single-file programs, including
    // identifiers that are reserved only by the opt-in module grammar.
    let graph = if larp_syntax::parse(&source).is_ok() {
        None
    } else {
        let opened = match project_context {
            Some(context) => project::open_project(Path::new(path), &context.project.root),
            None => project::open_entry(Path::new(path)),
        };
        let (entry, mut resolver) = match opened {
            Ok(entry) => entry,
            Err(message) => {
                let diagnostic = larp_diagnostics::Diagnostic::error(
                    "LARP-M001",
                    source.span(0..0).unwrap(),
                    message,
                );
                format.diagnostic(&diagnostic, std::slice::from_ref(&source));
                return ExitCode::from(1);
            }
        };
        match larp_compiler::project::load(entry, &mut resolver) {
            Ok(project) => Some(project),
            Err(failure) => {
                for diagnostic in failure.diagnostics {
                    format.diagnostic(&diagnostic, &failure.sources);
                }
                return ExitCode::from(1);
            }
        }
    };
    let sources = graph.as_ref().map_or_else(
        || std::slice::from_ref(&source),
        |project| project.sources(),
    );
    let result = match &graph {
        Some(project) => larp_compiler::compile_project(project),
        None => larp_compiler::compile(&source),
    };
    let compilation = match result {
        Ok(compilation) => compilation,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                format.diagnostic(&diagnostic, sources);
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
            if let Some(context) = project_context
                && std::fs::canonicalize(&output)
                    .is_ok_and(|output| output == context.project.manifest)
            {
                format.event(
                    "project_error",
                    Some("LARP-J003"),
                    "artifact output must differ from the project manifest",
                );
                return ExitCode::from(1);
            }
            if output == Path::new(path)
                || std::fs::canonicalize(&output).ok().is_some_and(|output| {
                    sources.iter().any(|source| {
                        std::fs::canonicalize(source.name()).is_ok_and(|input| input == output)
                    })
                })
            {
                if project_context.is_some() {
                    format.event(
                        "project_error",
                        Some("LARP-J003"),
                        "artifact output must differ from every loaded source file",
                    );
                    return ExitCode::from(1);
                }
                format.error("artifact output must differ from the source file");
                return ExitCode::from(2);
            }
            let bytes = match larp_artifact::encode(&compilation.bytecode, sources) {
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
            if let Some(context) = project_context
                && context.default_output
            {
                let target = context.project.root.join("target");
                let result = match std::fs::symlink_metadata(&target) {
                    Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                        Ok(())
                    }
                    Ok(_) => Err(std::io::Error::other(
                        "default target must be a directory, not a symlink",
                    )),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        std::fs::create_dir(&target)
                    }
                    Err(error) => Err(error),
                };
                if let Err(error) = result {
                    format.event(
                        "project_error",
                        Some("LARP-J003"),
                        &format!("cannot create project output directory: {error}"),
                    );
                    return ExitCode::from(1);
                }
            }
            if let Err(error) = write_file(&output, &bytes, None, None) {
                format.error(format_args!("cannot write '{}': {error}", output.display()));
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        mode => execute(&compilation.bytecode, sources, mode, format),
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
    let result = if larp_syntax::parse(source).is_ok() {
        larp_format::format(source)
    } else {
        larp_format::format_unit(source)
    };
    let formatted = match result {
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
