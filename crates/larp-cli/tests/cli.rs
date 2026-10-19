use std::process::{Command, Output};

fn larp(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_larp"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn help_reports_actual_implementation_status() {
    let output = larp(&["--help"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("verified bytecode and versioned artifacts")
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn explanation_has_code_and_actionable_conversion_help() {
    let output = larp(&["explain", "LARP-T041"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("LARP-T041"));
    assert!(text.contains("ascend<mf>"));
    assert!(text.contains("result<mf, ConversionError>"));
}

#[test]
fn unavailable_and_invalid_commands_fail_without_pretending_to_compile() {
    for args in [
        vec!["fmt", "missing.larp", "--unknown"],
        vec!["explain", "LARP-T999"],
        vec!["explain"],
        vec!["--help", "extra"],
    ] {
        let output = larp(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

fn fixture(name: &str) -> String {
    format!(
        "{}/../../tests/conformance/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[test]
fn check_compiles_without_observing_and_run_executes_real_source() {
    let file = fixture("pass/functions.larp");
    let checked = larp(&["check", &file]);
    assert!(checked.status.success());
    assert!(checked.stdout.is_empty());
    let output = larp(&["run", &file]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"120\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn compile_and_runtime_failures_have_nonzero_exit_codes_and_source_locations() {
    for (file, code) in [
        ("fail/truthiness.larp", "LARP-T002"),
        ("runtime/overflow.larp", "LARP-R001"),
    ] {
        let output = larp(&["run", &fixture(file)]);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(code));
        assert!(stderr.contains(" --> "));
        assert!(output.stdout.is_empty());
    }
    assert_eq!(larp(&["check", "missing.larp"]).status.code(), Some(1));
    assert_eq!(
        larp(&["run", &fixture("pass/scalars.larp"), "--fuel", "0"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        larp(&["run", &fixture("pass/scalars.larp"), "--fuel", "invalid"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn dumps_expose_the_compiler_artifacts() {
    let bytecode = larp(&["dump", &fixture("pass/functions.larp")]);
    assert!(bytecode.status.success());
    assert!(
        String::from_utf8(bytecode.stdout)
            .unwrap()
            .contains("Call(0)")
    );
    let ir = larp(&["dump", &fixture("pass/functions.larp"), "--ir"]);
    assert!(ir.status.success());
    assert!(String::from_utf8(ir.stdout).unwrap().contains("Branch"));
}

#[test]
fn gc_stress_runs_real_closures_and_composes_with_instruction_limits() {
    let file = fixture("pass/closure-gc.larp");
    for options in [
        vec!["--gc-stress"],
        vec!["--gc-stress", "--fuel", "100000"],
        vec!["--fuel", "100000", "--gc-stress"],
    ] {
        let mut args = vec!["run", &file];
        args.extend(options);
        let output = larp(&args);
        assert!(
            output.status.success(),
            "{:?}",
            String::from_utf8(output.stderr)
        );
        assert_eq!(output.stdout, b"49\n");
    }
    for options in [
        vec!["--gc-stress", "--gc-stress"],
        vec!["--fuel", "1", "--fuel", "2"],
        vec!["--fuel"],
        vec!["--gc-stress", "--unknown"],
    ] {
        let mut args = vec!["run", &file];
        args.extend(options);
        let output = larp(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn scenario_observations_use_stderr_and_check_does_not_execute_them() {
    let file = fixture("pass/scenario.larp");
    let checked = larp(&["check", &file]);
    assert!(
        checked.status.success(),
        "{:?}",
        String::from_utf8(checked.stderr.clone())
    );
    assert!(checked.stdout.is_empty());
    assert!(checked.stderr.is_empty());
    for options in [vec![], vec!["--gc-stress"]] {
        let mut args = vec!["run", &file];
        args.extend(options);
        let output = larp(&args);
        assert!(
            output.status.success(),
            "{:?}",
            String::from_utf8(output.stderr.clone())
        );
        assert_eq!(output.stdout, b"20\n3\n");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "PALANTIR [PeterThiel]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\nvaluation | 20 | 1000000000 | 1000000000 | false\naura | 0 | 3 | - | true\n"
        );
    }
}

#[test]
fn lazy_values_execute_once_and_suppression_does_not_hide_runtime_diagnostics() {
    for file in ["pass/lazy-capture.larp", "pass/lazy-suppression.larp"] {
        let file = fixture(file);
        let checked = larp(&["check", &file]);
        assert!(checked.status.success());
        assert!(checked.stdout.is_empty());
        assert!(checked.stderr.is_empty());
        for options in [vec![], vec!["--gc-stress"]] {
            let mut args = vec!["run", &file];
            args.extend(options);
            let output = larp(&args);
            assert!(
                output.status.success(),
                "{:?}",
                String::from_utf8_lossy(&output.stderr)
            );
            let expected = if file.ends_with("lazy-capture.larp") {
                b"4\n4\n".as_slice()
            } else {
                b"49\n49\ncallee\n".as_slice()
            };
            assert_eq!(output.stdout, expected);
            assert!(output.stderr.is_empty());
        }
    }
    let failed = larp(&["run", &fixture("runtime/suppressed-abort.larp")]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(failed.stdout.is_empty());
    let diagnostic = String::from_utf8(failed.stderr).unwrap();
    assert!(diagnostic.contains("error[LARP-R003]"));
    assert!(diagnostic.contains("still fatal"));
    assert!(diagnostic.contains(" --> "));
    let dump = larp(&["dump", &fixture("pass/lazy-capture.larp")]);
    assert!(dump.status.success());
    let dump = String::from_utf8(dump.stdout).unwrap();
    assert!(dump.contains("MakeLazy("));
    assert!(dump.contains("Force("));
}

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "larp-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn built_program_runs_without_source_and_preserves_embedded_runtime_locations() {
    let scratch = Scratch::new();
    let source = scratch.path("unicode.larp");
    let artifact = scratch.path("unicode.larpbc");
    std::fs::write(&source, "main_character { yap(\"🦀\"); no_cap(cap); }\n").unwrap();
    let expected = larp(&["run", &source]);
    assert_eq!(expected.status.code(), Some(1));
    let built = larp(&["build", &source]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(built.stdout.is_empty());
    assert!(built.stderr.is_empty());
    let bytes = std::fs::read(&artifact).unwrap();
    assert_eq!(&bytes[..8], b"LARPBC\r\n");
    let second = scratch.path("same.larpbc");
    assert!(larp(&["build", &source, "-o", &second]).status.success());
    assert_eq!(bytes, std::fs::read(&second).unwrap());
    std::fs::remove_file(&source).unwrap();
    let checked = larp(&["check", &artifact]);
    assert!(checked.status.success());
    assert!(checked.stdout.is_empty() && checked.stderr.is_empty());
    for options in [vec![], vec!["--gc-stress"]] {
        let mut args = vec!["run", &artifact];
        args.extend(options);
        let loaded = larp(&args);
        assert_eq!(loaded.status.code(), expected.status.code());
        assert_eq!(loaded.stdout, expected.stdout);
        assert_eq!(loaded.stderr, expected.stderr);
    }
    let renamed = scratch.path("renamed.data");
    std::fs::rename(&artifact, &renamed).unwrap();
    assert_eq!(larp(&["run", &renamed]).stderr, expected.stderr);
    assert!(larp(&["dump", &renamed]).status.success());
    for args in [vec!["dump", &renamed, "--ir"], vec!["build", &renamed]] {
        assert_eq!(larp(&args).status.code(), Some(2));
    }
}

#[test]
fn json_diagnostics_keep_types_help_and_related_declaration_labels() {
    let output = larp(&[
        "--diagnostic-format=json",
        "check",
        &fixture("fail/narrowing.larp"),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("\"kind\":\"diagnostic\""));
    assert!(text.contains("\"code\":\"LARP-T041\""));
    assert!(text.contains("\"expected_type\":\"mf\",\"found_type\":\"unc\""));
    assert!(text.contains("ascend<mf>"));
    let scratch = Scratch::new();
    let source = scratch.path("duplicate.larp");
    std::fs::write(&source, "main_character { mf x = 1; mf x = 2; }").unwrap();
    let output = larp(&["--diagnostic-format=json", "check", &source]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("\"code\":\"LARP-N002\""));
    assert!(!text.contains("\"related\":[]"));
}

#[test]
fn json_runtime_locations_survive_deleted_unicode_crlf_source() {
    let scratch = Scratch::new();
    let source = scratch.path("quote\"\n🦀.larp");
    let artifact = scratch.path("program.larpbc");
    let code = "main_character {\r\n\tyap(\"🦀\"); crashout(\"bad\\nvalue\\t\");\r\n}\r\n";
    std::fs::write(&source, code).unwrap();
    let expected = larp(&["--diagnostic-format=json", "run", &source]);
    assert_eq!(expected.status.code(), Some(1));
    assert_eq!(expected.stdout, "🦀\n".as_bytes());
    let text = String::from_utf8(expected.stderr.clone()).unwrap();
    assert_eq!(text.bytes().filter(|byte| *byte == b'\n').count(), 1);
    assert!(text.contains("quote\\\"\\n🦀.larp"));
    assert!(text.contains("\"message\":\"bad\\nvalue\\t\""));
    let start = code.find("crashout").unwrap();
    assert!(text.contains(&format!(
        "\"start\":{start},\"end\":{}",
        start + "crashout(\"bad\\nvalue\\t\")".len()
    )));
    assert!(text.contains("\"start_location\":{\"line\":2,\"column\":12}"));
    assert!(
        larp(&[
            "--diagnostic-format=json",
            "build",
            &source,
            "-o",
            &artifact
        ])
        .status
        .success()
    );
    std::fs::remove_file(&source).unwrap();
    let loaded = larp(&["--diagnostic-format=json", "run", &artifact, "--gc-stress"]);
    assert_eq!(loaded.status.code(), Some(1));
    assert_eq!(loaded.stdout, expected.stdout);
    assert_eq!(loaded.stderr, expected.stderr);
}

#[test]
fn json_cli_artifact_and_formatter_failures_remain_machine_readable() {
    let scratch = Scratch::new();
    let source = scratch.path("format.larp");
    let artifact = scratch.path("bad.larpbc");
    std::fs::write(&source, "main_character{yap(49);}").unwrap();
    std::fs::write(&artifact, b"invalid artifact").unwrap();
    for (args, status, kind) in [
        (vec!["check", "missing.larp"], 1, "cli_error"),
        (vec!["run", &source, "--unknown"], 2, "cli_error"),
        (
            vec!["--diagnostic-format=text", "check", &source],
            2,
            "cli_error",
        ),
        (vec!["check", &artifact], 1, "artifact_error"),
        (vec!["fmt", &source, "--check"], 1, "format_needed"),
    ] {
        let mut command = vec!["--diagnostic-format=json"];
        command.extend(args);
        let output = larp(&command);
        assert_eq!(output.status.code(), Some(status));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.contains(&format!("\"kind\":\"{kind}\"")), "{text}");
        assert_eq!(text.bytes().filter(|byte| *byte == b'\n').count(), 1);
    }
    std::fs::write(&source, "main_character { mf value = ; }").unwrap();
    let output = larp(&["--diagnostic-format=json", "fmt", &source, "--stdout"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("\"kind\":\"diagnostic\"")
    );
    std::fs::write(&source, "main_character { yap(args()[0]); }").unwrap();
    let output = larp(&[
        "--diagnostic-format=json",
        "run",
        &source,
        "--",
        "--diagnostic-format=text",
    ]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"--diagnostic-format=text\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn build_failure_preserves_prior_output_and_rejects_source_overwrite() {
    let scratch = Scratch::new();
    let source = scratch.path("program.larp");
    let artifact = scratch.path("program.larpbc");
    let valid = b"main_character { yap(49); }";
    std::fs::write(&source, valid).unwrap();
    assert!(larp(&["build", &source]).status.success());
    let bytes = std::fs::read(&artifact).unwrap();
    assert_eq!(
        larp(&["build", &source, "-o", &source]).status.code(),
        Some(2)
    );
    assert_eq!(std::fs::read(&source).unwrap(), valid);
    assert_eq!(std::fs::read(&artifact).unwrap(), bytes);
    std::fs::write(&source, "main_character { if (49) {} }").unwrap();
    let failed = larp(&["build", &source]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("LARP-T002"));
    assert_eq!(std::fs::read(&artifact).unwrap(), bytes);
    assert_eq!(larp(&["run", &artifact]).stdout, b"49\n");
    std::fs::write(&source, valid).unwrap();
    let missing_parent = scratch.path("missing/output.larpbc");
    assert_eq!(
        larp(&["build", &source, "-o", &missing_parent])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 2);
    assert_eq!(larp(&["build", "missing.larp"]).status.code(), Some(1));
    assert_eq!(larp(&["build", &source, "-o"]).status.code(), Some(2));
}

#[test]
fn artifact_loading_rejects_corruption_and_obeys_runtime_fuel() {
    let scratch = Scratch::new();
    let source = fixture("pass/scenario.larp");
    let artifact = scratch.path("program.larpbc");
    assert!(larp(&["build", &source, "-o", &artifact]).status.success());
    let valid = std::fs::read(&artifact).unwrap();
    let exhausted = larp(&["run", &artifact, "--fuel", "0", "--gc-stress"]);
    assert_eq!(exhausted.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&exhausted.stderr).contains("LARP-R006"));
    assert!(exhausted.stdout.is_empty());
    let mut bad_version = valid.clone();
    bad_version[8] = 255;
    let mut bad_length = valid.clone();
    bad_length[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut trailing = valid.clone();
    trailing.push(0);
    for bytes in [valid[..7].to_vec(), bad_version, bad_length, trailing] {
        std::fs::write(&artifact, bytes).unwrap();
        let output = larp(&["run", &artifact]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("LARP-B002") || stderr.contains("LARP-B003"),
            "{stderr}"
        );
    }
    std::fs::write(&artifact, "main_character { yap(49); }").unwrap();
    assert_eq!(larp(&["check", &artifact]).status.code(), Some(1));
    for code in ["LARP-B002", "LARP-B003"] {
        assert!(larp(&["explain", code]).status.success());
    }
}

#[test]
fn formatter_modes_are_idempotent_and_check_stdout_never_modify_source() {
    let scratch = Scratch::new();
    let source = scratch.path("format.larp");
    let raw = "// lead 🦀\nmain_character{mf n=4;yap(n);}// tail\n";
    std::fs::write(&source, raw).unwrap();
    let checked = larp(&["fmt", &source, "--check"]);
    assert_eq!(checked.status.code(), Some(1));
    assert!(checked.stdout.is_empty());
    assert!(String::from_utf8_lossy(&checked.stderr).contains("needs formatting"));
    assert_eq!(std::fs::read_to_string(&source).unwrap(), raw);
    let preview = larp(&["fmt", &source, "--stdout"]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert!(preview.stderr.is_empty());
    let formatted = String::from_utf8(preview.stdout).unwrap();
    assert!(formatted.contains("\n    mf n = 4;\n"), "{formatted}");
    assert!(formatted.contains("// lead 🦀") && formatted.contains("// tail"));
    assert_eq!(std::fs::read_to_string(&source).unwrap(), raw);
    let written = larp(&["fmt", &source]);
    assert!(
        written.status.success(),
        "{}",
        String::from_utf8_lossy(&written.stderr)
    );
    assert!(written.stdout.is_empty() && written.stderr.is_empty());
    assert_eq!(std::fs::read_to_string(&source).unwrap(), formatted);
    let stable = larp(&["fmt", "--check", &source]);
    assert!(stable.status.success());
    assert!(stable.stdout.is_empty() && stable.stderr.is_empty());
    assert_eq!(
        larp(&["fmt", "--stdout", &source]).stdout,
        formatted.as_bytes()
    );
    assert_eq!(larp(&["run", &source]).stdout, b"4\n");
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 1);
}

#[test]
fn formatter_requires_valid_syntax_but_does_not_require_valid_types() {
    let scratch = Scratch::new();
    let source = scratch.path("unfinished.larp");
    let semantic_error = "main_character{if(49){yap(\"never\");}}";
    std::fs::write(&source, semantic_error).unwrap();
    assert!(larp(&["fmt", &source]).status.success());
    let checked = larp(&["check", &source]);
    assert!(String::from_utf8_lossy(&checked.stderr).contains("LARP-T002"));
    for raw in ["main_character{yap(49)", "main_character{yap(\"bad\\q\");}"] {
        std::fs::write(&source, raw).unwrap();
        for options in [vec![], vec!["--check"], vec!["--stdout"]] {
            let mut args = vec!["fmt", &source];
            args.extend(options);
            let failed = larp(&args);
            assert_eq!(failed.status.code(), Some(1));
            assert!(failed.stdout.is_empty());
            assert!(String::from_utf8_lossy(&failed.stderr).contains(" --> "));
            assert_eq!(std::fs::read_to_string(&source).unwrap(), raw);
        }
    }
    for args in [
        vec!["fmt"],
        vec!["fmt", "--check"],
        vec!["fmt", "--stdout", "--check"],
        vec!["fmt", &source, "--stdout", "--check"],
    ] {
        assert_eq!(larp(&args).status.code(), Some(2), "{args:?}");
    }
    assert_eq!(larp(&["fmt", "missing.larp"]).status.code(), Some(1));
    let artifact = scratch.path("program.larpbc");
    assert!(
        larp(&["build", &fixture("pass/functions.larp"), "-o", &artifact])
            .status
            .success()
    );
    let before = std::fs::read(&artifact).unwrap();
    assert_eq!(larp(&["fmt", &artifact]).status.code(), Some(2));
    assert_eq!(std::fs::read(&artifact).unwrap(), before);
    assert!(larp(&["explain", "LARP-C001"]).status.success());
}

#[cfg(unix)]
#[test]
fn formatter_preserves_source_permissions_and_follows_symlink_targets() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let scratch = Scratch::new();
    let target = scratch.path("target.larp");
    let link = scratch.path("link.larp");
    std::fs::write(&target, "main_character{yap(49);}").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&target, &link).unwrap();
    assert!(larp(&["fmt", &link]).status.success());
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(larp(&["fmt", &target, "--check"]).status.success());
    assert_eq!(larp(&["run", &link]).stdout, b"49\n");
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 2);
}

#[test]
fn formatter_input_size_is_bounded_before_utf8_decoding_and_source_indexing() {
    let scratch = Scratch::new();
    let path = scratch.path("oversize.larp");
    let text = format!(
        "main_character {{}} //{}",
        "🦀".repeat(larp_format::MAX_SOURCE_BYTES / 4)
    );
    std::fs::write(&path, &text).unwrap();
    let before = std::fs::metadata(&path).unwrap().len();
    for options in [vec![], vec!["--check"], vec!["--stdout"]] {
        let mut args = vec!["fmt", &path];
        args.extend(options);
        let output = larp(&args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("LARP-P003") && stderr.contains("source size limit"),
            "{stderr}"
        );
        assert_eq!(std::fs::metadata(&path).unwrap().len(), before);
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 1);
}

fn larp_with_input(args: &[&str], input: &[u8]) -> Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_larp"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn cli_stdin_and_program_arguments_survive_build_format_and_deleted_source() {
    let scratch = Scratch::new();
    let source = scratch.path("input.larp");
    let artifact = scratch.path("input.larpbc");
    std::fs::copy(fixture("pass/host-input.larp"), &source).unwrap();
    let checked = larp_with_input(&["check", &source], b"not executed");
    assert!(checked.status.success());
    assert!(checked.stdout.is_empty() && checked.stderr.is_empty());
    let formatted = larp(&["fmt", &source]);
    assert!(formatted.status.success());
    let built = larp_with_input(&["build", &source], b"not executed");
    assert!(built.status.success());
    assert!(built.stdout.is_empty() && built.stderr.is_empty());
    let args = [
        "run",
        &source,
        "--gc-stress",
        "--fuel",
        "10000",
        "--",
        "49",
        "🦀",
        "--fuel",
    ];
    let expected = larp_with_input(&args, b"one\r\n\n");
    assert!(
        expected.status.success(),
        "{}",
        String::from_utf8_lossy(&expected.stderr)
    );
    assert_eq!(expected.stdout, "49\n🦀\n--fuel\none\n\nEOF\n".as_bytes());
    assert!(expected.stderr.is_empty());
    std::fs::remove_file(&source).unwrap();
    let loaded = larp_with_input(
        &["run", &artifact, "--gc-stress", "--", "49", "🦀", "--fuel"],
        b"one\r\n\n",
    );
    assert!(loaded.status.success());
    assert_eq!(loaded.stdout, expected.stdout);
    assert_eq!(loaded.stderr, expected.stderr);
}

#[test]
fn cli_utf8_errors_are_result_data_but_input_resource_exhaustion_is_fatal() {
    let scratch = Scratch::new();
    let source = scratch.path("input.larp");
    std::fs::write(&source, "main_character{yap(readLine().cope(err=>some(err)).copium(\"EOF\"));yap(readLine().cope(err=>some(err)).copium(\"EOF\"));}").unwrap();
    let output = larp_with_input(&["run", &source], b"\xff\nnext\n");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"input is not valid UTF-8\nnext\n");
    assert!(output.stderr.is_empty());
    let oversized = vec![b'x'; larp_vm::MAX_LINE_BYTES + 1];
    let failed = larp_with_input(&["run", &source], &oversized);
    assert_eq!(failed.status.code(), Some(1));
    assert!(failed.stdout.is_empty());
    let diagnostic = String::from_utf8(failed.stderr).unwrap();
    assert!(
        diagnostic.contains("LARP-R006") && diagnostic.contains(" --> "),
        "{diagnostic}"
    );
    std::fs::write(&source, "main_character{yap(args().size());}").unwrap();
    assert_eq!(larp(&["run", &source]).stdout, b"0\n");
    assert_eq!(larp(&["run", &source, "--"]).stdout, b"0\n");
    assert_eq!(larp(&["run", &source, "--", ""]).stdout, b"1\n");
    assert_eq!(larp(&["run", &source, "--", "--unknown"]).stdout, b"1\n");
    let mut arguments = vec!["run", &source, "--"];
    arguments.extend(std::iter::repeat_n("", larp_vm::MAX_ARGUMENTS + 1));
    let failed = larp(&arguments);
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("LARP-R006"));
}
