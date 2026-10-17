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
            .contains("No LARP compiler")
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
        vec!["run", "missing.larp"],
        vec!["check", "missing.larp"],
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
