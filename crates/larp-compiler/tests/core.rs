use larp_compiler::compile;
use larp_diagnostics::Diagnostic;
use larp_source::{SourceFile, SourceId};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "test.larp", text)
}

fn run(text: &str) -> String {
    let compilation = compile(&source(text)).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let mut output = Vec::new();
    larp_vm::execute(&compilation.bytecode, &mut output, Default::default()).unwrap();
    String::from_utf8(output).unwrap()
}

fn rejects(text: &str, code: &str) {
    let errors = compile(&source(text))
        .err()
        .expect("program must fail compilation");
    assert!(
        errors.iter().any(|diagnostic| diagnostic.code.id == code),
        "{errors:#?}"
    );
    assert!(
        errors
            .iter()
            .all(|diagnostic| diagnostic.code.id != "LARP-B001"),
        "frontend error reached invalid bytecode"
    );
}

fn runtime_error(text: &str) -> Box<Diagnostic> {
    let compilation = compile(&source(text)).unwrap_or_else(|errors| panic!("{errors:#?}"));
    larp_vm::execute(&compilation.bytecode, &mut Vec::new(), Default::default()).unwrap_err()
}

#[test]
fn forward_calls_mutual_recursion_and_return_paths_execute_in_vm() {
    assert_eq!(
        run(r#"
        sidequest even(mf n) -> verdict { if (n == 0) { return fr; } return odd(n - 1); }
        sidequest odd(mf n) -> verdict { if (n == 0) { return cap; } return even(n - 1); }
        main_character { yap(even(10)); yap(odd(9)); }
    "#),
        "fr\nfr\n"
    );
    assert_eq!(
        run(r#"
        sidequest choose(verdict v) -> text uses() {
            if (v) { return "a"; } else { return "b"; }
        }
        main_character { yap(choose(fr)); yap(choose(cap)); }
    "#),
        "a\nb\n"
    );
}

#[test]
fn lexical_scopes_initializer_visibility_and_else_if_are_predictable() {
    assert_eq!(
        run(r#"
        main_character {
            mf x = 2;
            { mf x = x + 1; yap(x); }
            if (cap) { x = 4; } else if (x == 2) { x = 9; } else { x = 5; }
            yap(x);
        }
    "#),
        "3\n9\n"
    );
    rejects(
        "main_character { { mf hidden = 1; } yap(hidden); }",
        "LARP-N001",
    );
    rejects("main_character { mf x = x; }", "LARP-N001");
}

#[test]
fn loop_breaks_and_integer_boundary_ranges_do_not_overflow() {
    assert_eq!(
        run(r#"
        main_character {
            npc loop (mf i in 2_147_483_646..2_147_483_647) { yap(i); }
            npc loop (unc i in 9_223_372_036_854_775_806..9_223_372_036_854_775_807) { yap(i); }
            npc loop (mf i in 3..1) { crashout("empty range executed"); }
            mf count = 0;
            goon {
                npc loop (mf i in 0..10) { count += 1; touch_grass; }
                if (count == 3) { touch_grass; }
            }
            yap(count);
        }
    "#),
        "2147483646\n9223372036854775806\n3\n"
    );
    rejects(
        "main_character { npc loop (mf i in 0..3) { i = 2; } }",
        "LARP-T008",
    );
    rejects("main_character { touch_grass; }", "LARP-T009");
}

#[test]
fn argument_operand_and_range_bound_evaluation_is_left_to_right() {
    assert_eq!(
        run(r#"
        sidequest mark(mf value) -> mf uses(observe) { yap(value); return value; }
        sidequest pair(mf a, mf b) -> mf { return a * 10 + b; }
        main_character {
            yap(pair(mark(1), mark(2)));
            npc loop (mf i in mark(0)..mark(2)) { yap(i); }
        }
    "#),
        "1\n2\n12\n0\n2\n0\n1\n"
    );
}

#[test]
fn context_types_literals_but_never_converts_existing_numbers() {
    assert_eq!(
        run(r#"
        sidequest large() -> unc { return 2_147_483_647 + 1; }
        main_character { unc x = -9_223_372_036_854_775_808; yap(x); yap(large()); }
    "#),
        "-9223372036854775808\n2147483648\n"
    );
    rejects(
        "main_character { unc large = 1; mf small = large; }",
        "LARP-T041",
    );
    rejects(
        "main_character { mf a = 1; unc b = 2; yap(a + b); }",
        "LARP-T001",
    );
    rejects("main_character { geeked x = 1; }", "LARP-T001");
    rejects("main_character { let x = 2_147_483_648; }", "LARP-T004");
}

#[test]
fn checked_dynamic_arithmetic_traps_and_remainder_minimum_is_zero() {
    for (expression, code) in [
        ("a + b", "LARP-R001"),
        ("a / 0", "LARP-R002"),
        ("a % 0", "LARP-R002"),
    ] {
        let text =
            format!("main_character {{ mf a = 2_147_483_647; mf b = 1; yap({expression}); }}");
        assert_eq!(runtime_error(&text).code.id, code);
    }
    assert_eq!(
        run(
            "main_character { mf a = -2_147_483_648; unc b = -9_223_372_036_854_775_808; yap(a % -1); yap(b % -1); }"
        ),
        "0\n0\n"
    );
    assert_eq!(
        runtime_error("main_character { unc a = -9_223_372_036_854_775_808; yap(a / -1); }")
            .code
            .id,
        "LARP-R001"
    );
    assert_eq!(
        runtime_error("main_character { unc a = -9_223_372_036_854_775_808; yap(-a); }")
            .code
            .id,
        "LARP-R001"
    );
}

#[test]
fn short_circuit_skips_runtime_and_constant_evaluation_but_not_typing() {
    assert_eq!(
        run(r#"
        sidequest forbidden() -> verdict { crashout("called"); }
        main_character {
            no_cap(fr || forbidden());
            no_cap(!(cap && forbidden()));
            no_cap(fr || (1 / 0 == 0));
            no_cap(!(cap && (1 / 0 == 0)));
            yap("safe");
        }
    "#),
        "safe\n"
    );
    rejects("main_character { yap(fr || 1); }", "LARP-T001");
    rejects("main_character { yap(cap && missing); }", "LARP-N001");
    rejects("main_character { yap(1 / 0); }", "LARP-T004");
    rejects("main_character { yap(2_147_483_647 + 1); }", "LARP-T004");
}

#[test]
fn floating_point_and_immutable_text_operations_work() {
    assert_eq!(
        run(r#"
        main_character {
            geeked zero = 0.0;
            geeked nan = zero / zero;
            yap(1.0 / zero); yap(-1.0 / zero); yap(nan); yap(nan == nan);
            yap("hello " + "🦀"); yap(1.25e2);
        }
    "#),
        "inf\n-inf\nnan\ncap\nhello 🦀\n125\n"
    );
    rejects("main_character { geeked x = 1.0e999; }", "LARP-T004");
}

#[test]
fn types_scopes_signatures_and_dead_code_are_checked_before_execution() {
    for (text, code) in [
        ("main_character { if (1) {} }", "LARP-T002"),
        ("main_character { locked_in while (1) {} }", "LARP-T002"),
        ("main_character { mf x = 0; mf x = 1; }", "LARP-N002"),
        (
            "sidequest f(mf a, mf a) -> unit {} main_character {}",
            "LARP-N002",
        ),
        (
            "sidequest f() -> unit {} sidequest f() -> unit {} main_character {}",
            "LARP-N002",
        ),
        (
            "sidequest f(verdict flag) -> mf { if (flag) { return 1; } } main_character {}",
            "LARP-T007",
        ),
        (
            "sidequest f(mf x) -> mf { return x; } main_character { f(); }",
            "LARP-T006",
        ),
        ("main_character { if (cap) { yap(missing); } }", "LARP-N001"),
        ("main_character { return; mf wrong = cap; }", "LARP-T001"),
        ("main_character { unit x = yap(1); }", "LARP-T001"),
        ("main_character { Unknown x = 1; }", "LARP-T005"),
        ("main_character { mf f = 1; f(); }", "LARP-T001"),
        ("main_character { return 1; }", "LARP-T001"),
        ("archetype A {} main_character {}", "LARP-T011"),
    ] {
        rejects(text, code);
    }
}

#[test]
fn effects_are_checked_transitively_through_declared_call_bounds() {
    assert_eq!(
        run("sidequest pure() -> mf uses() { return 2 + 3; } main_character { yap(pure()); }"),
        "5\n"
    );
    rejects(
        "sidequest noisy() -> unit { yap(1); } main_character {}",
        "LARP-E001",
    );
    rejects(
        "sidequest inner() -> mf uses(observe) { yap(1); return 0; } sidequest outer() -> mf uses() { return inner(); } main_character {}",
        "LARP-E001",
    );
    rejects(
        "sidequest f(mf x) -> mf uses() { return x + 1; } main_character {}",
        "LARP-E001",
    );
    rejects(
        "sidequest f() -> unit uses(unknown) {} main_character {}",
        "LARP-E003",
    );
}

#[test]
fn runtime_limits_bound_infinite_loops_and_recursion_without_rust_recursion() {
    for text in [
        "main_character { goon {} }",
        "sidequest recur() -> unit { recur(); } main_character { recur(); }",
    ] {
        let compilation = compile(&source(text)).unwrap_or_else(|errors| panic!("{errors:#?}"));
        let error = larp_vm::execute(
            &compilation.bytecode,
            &mut Vec::new(),
            larp_vm::Limits {
                instructions: 10_000,
                call_frames: 32,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.code.id, "LARP-R006");
    }
}

#[test]
fn output_failure_and_explicit_fatal_checks_are_structured_errors() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let compilation = compile(&source("main_character { yap(1); }"))
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    assert_eq!(
        larp_vm::execute(&compilation.bytecode, &mut Broken, Default::default())
            .unwrap_err()
            .code
            .id,
        "LARP-R007"
    );
    assert_eq!(
        runtime_error("main_character { no_cap(cap); }").code.id,
        "LARP-R003"
    );
    assert_eq!(
        runtime_error("main_character { crashout(\"stop\"); }").message,
        "stop"
    );
}

#[test]
fn diagnostics_retain_exact_utf8_spans_and_render_scalar_columns() {
    let source = source("main_character { text name = \"🦀\"; mf wrong = cap; }");
    let errors = compile(&source).err().unwrap();
    let error = &errors[0];
    assert_eq!(source.slice(error.primary.span), Some("cap"));
    let location = source.location(error.primary.span.range().start).unwrap();
    let prefix = source.text().split("cap").next().unwrap();
    assert_eq!(location.column, prefix.chars().count() + 1);
    let rendered = larp_diagnostics::render(error, &source);
    assert!(rendered.contains("LARP-T001"));
    assert!(rendered.contains("expected: mf; found: verdict"));
}

#[test]
fn bytecode_and_typed_control_flow_exist_before_vm_execution() {
    let compilation = compile(&source(
        "sidequest add(mf x) -> mf { return x + 1; } main_character { yap(add(2)); }",
    ))
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let dump = compilation.bytecode.disassemble();
    assert!(dump.contains("Binary(Add, Mf)"));
    assert!(dump.contains("Call(0)"));
    assert_eq!(compilation.ir.functions[0].locals[0], larp_types::Type::Mf);
    assert!(compilation.ir.functions[0].blocks.len() > 1);
}
