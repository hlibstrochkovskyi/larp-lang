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
            .contains("verified in-memory bytecode")
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
        vec!["build", "missing.larp"],
        vec!["fmt", "missing.larp"],
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
