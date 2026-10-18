use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};
use std::io::{self, Write};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "lazy-and-suppression.larp", text)
}

fn compile_source(source: &SourceFile) -> larp_compiler::Compilation {
    compile(source).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(|error| larp_diagnostics::render(error, source))
                .collect::<String>()
        )
    })
}

fn run(text: &str, expected: &str, expected_inspection: &str) {
    let source = source(text);
    let compilation = compile_source(&source);
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let mut inspection = Vec::new();
        let stats = larp_vm::execute_with_observers(
            &compilation.bytecode,
            &mut output,
            &mut inspection,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output, expected.as_bytes(), "gc_stress={gc_stress}");
        assert_eq!(
            inspection,
            expected_inspection.as_bytes(),
            "gc_stress={gc_stress}"
        );
        assert_eq!(stats.allocations, stats.reclaimed);
    }
}

fn rejects(text: &str, code: &str) {
    let source = source(text);
    let errors = compile(&source)
        .err()
        .expect("Expected a frontend rejection");
    assert!(
        errors.iter().any(|error| error.code.id == code),
        "{}",
        errors
            .iter()
            .map(|error| larp_diagnostics::render(error, &source))
            .collect::<String>()
    );
    assert!(
        errors.iter().all(|error| error.code.id != "LARP-B001"),
        "Source errors must not reach the bytecode verifier"
    );
    assert!(
        errors
            .iter()
            .all(|error| source.slice(error.primary.span).is_some())
    );
}

fn runtime_error(text: &str, code: &str, span: &str, expected_output: &str) {
    let source = source(text);
    let compilation = compile_source(&source);
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let diagnostic = larp_vm::execute(
            &compilation.bytecode,
            &mut output,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(diagnostic.code.id, code);
        assert_eq!(source.slice(diagnostic.primary.span), Some(span));
        assert_eq!(output, expected_output.as_bytes());
    }
}

#[test]
fn existing_lazy_fixtures_and_example_execute_or_reject_with_their_real_semantics() {
    run(
        include_str!("../../../tests/conformance/pass/lazy-capture.larp"),
        "4\n4\n",
        "",
    );
    rejects(
        include_str!("../../../tests/conformance/fail/lazy-observation.larp"),
        "LARP-E001",
    );
    run(include_str!("../../../examples/lazy.larp"), "49\n49\n", "");
}

#[test]
fn unused_fatal_initializers_are_never_evaluated_and_forcing_preserves_expression_spans() {
    run(
        "main_character { mf zero = 0; nonchalant mf never = 1 / zero; yap(49); }",
        "49\n",
        "",
    );
    runtime_error(
        "main_character { mf zero = 0; nonchalant mf value = 1 / zero; yap(7); yap(value); }",
        "LARP-R002",
        "1 / zero",
        "7\n",
    );
    runtime_error(
        "main_character { nonchalant mf value = crashout(\"lazy fatal\"); yap(7); yap(value); }",
        "LARP-R003",
        "crashout(\"lazy fatal\")",
        "7\n",
    );
}

#[test]
fn repeated_forcing_reuses_the_memoized_value_with_a_bounded_instruction_budget() {
    let source = source(
        r#"
        sidequest work(mf count) -> mf uses(abort) {
            mf total = 0;
            npc loop (mf i in 0..count) { total += 1; }
            return total;
        }
        main_character {
            nonchalant mf value = work(1000);
            mf total = 0;
            npc loop (mf i in 0..200) { total += value; }
            yap(total);
        }
    "#,
    );
    let compilation = compile_source(&source);
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let stats = larp_vm::execute_with_stats(
            &compilation.bytecode,
            &mut output,
            larp_vm::Limits {
                instructions: 100_000,
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output, b"200000\n");
        assert!(stats.instructions < 100_000);
        assert_eq!(stats.allocations, stats.reclaimed);
    }
}

#[test]
fn lazy_captures_snapshot_values_at_declaration_including_nested_fixed_arrays() {
    run(
        r#"
        main_character {
            mf base = 2;
            hediJeans<hediJeans<mf,2>,1> rows = [[3,4]];
            baddie<mf> optional = some(5);
            nonchalant mf total = base + rows[0][1] + optional.copium(0);
            base = 99; rows[0][1] = 99; optional = some(99);
            yap(total); yap(total);
            nonchalant hediJeans<mf,2> copied = rows[0];
            rows[0][0] = 100;
            yap(copied[0]); yap(copied[1]);
        }
    "#,
        "11\n11\n3\n99\n",
        "",
    );
}

#[test]
fn ordinary_closure_capture_and_later_lazy_capture_force_existing_lazy_bindings_immediately() {
    runtime_error(
        "main_character { mf zero = 0; nonchalant mf first = 1 / zero; yap(1); let unused = () => first; yap(2); }",
        "LARP-R002",
        "1 / zero",
        "1\n",
    );
    runtime_error(
        "main_character { mf zero = 0; nonchalant mf first = 1 / zero; yap(1); nonchalant mf unused = first + 1; yap(2); }",
        "LARP-R002",
        "1 / zero",
        "1\n",
    );
    run(
        "main_character { mf base = 2; nonchalant mf first = base + 1; nonchalant mf second = first + 1; let captured = () => second; base = 99; yap(first); yap(second); yap(captured()); }",
        "3\n4\n4\n",
        "",
    );
}

#[test]
fn named_pure_calls_function_references_and_nested_expected_results_are_valid_initializers() {
    run(
        r#"
        sidequest identity(mf value) -> mf uses() { return value; }
        sidequest pure() -> mf uses() { nonchalant mf value = identity(49); return value; }
        main_character {
            nonchalant result<baddie<hediJeans<mf,2>>,text> nested = win(some([2,3]));
            let selected = nested.cope(err => no_bitches).copium([0,0]);
            yap(selected[0]); yap(selected[1]);
            nonchalant mf called = identity(7); yap(called);
            nonchalant mf referenced = (() => identity)()(8); yap(referenced);
            nonchalant mf indirect = ((mf x) => x + 1)(20); yap(indirect);
            yap(pure());
        }
    "#,
        "2\n3\n7\n8\n21\n49\n",
        "",
    );
}

#[test]
fn lazy_effect_bounds_are_checked_at_initialization_and_at_every_force() {
    rejects(
        "sidequest noisy() -> mf uses(observe) { yap(1); return 1; } main_character { nonchalant mf value = noisy(); }",
        "LARP-E001",
    );
    rejects(
        "sidequest external() -> mf uses(io) { return 1; } main_character { nonchalant mf value = external(); }",
        "LARP-E001",
    );
    rejects(
        "sidequest read(mf x) -> mf uses(read) { return x; } main_character { nonchalant mf value = read(1); }",
        "LARP-E001",
    );
    rejects(
        "sidequest mutate(mf x) -> mf uses(write) { return x; } main_character { nonchalant mf value = mutate(1); }",
        "LARP-E001",
    );
    rejects(
        "sidequest pure(mf x) -> mf uses() { nonchalant mf value = x + 1; return value; } main_character {}",
        "LARP-E001",
    );
    run(
        "sidequest pure() -> mf uses() { nonchalant mf value = 49; return value; } main_character { yap(pure()); }",
        "49\n",
        "",
    );
    run(
        "sidequest unused(mf x) -> mf uses() { nonchalant mf value = x + 1; return 49; } main_character { yap(unused(0)); }",
        "49\n",
        "",
    );
}

#[test]
fn lazy_results_and_captures_require_snapshot_values_without_reference_alias_shortcuts() {
    for body in [
        "mfs<mf> values = [1]; nonchalant mf value = values[0];",
        "let object = new Box(); nonchalant mf value = object.value;",
        "let callback = () => 1; nonchalant mf value = callback();",
        "let object = new Box(); let alias = object; nonchalant mf value = alias.value;",
        "nonchalant mfs<mf> values = [];",
        "nonchalant Box object = new Box();",
        "nonchalant fn()->mf uses() callback = () => 1;",
        "nonchalant baddie<mfs<mf>> values = no_bitches;",
    ] {
        rejects(
            &format!(
                "archetype Box {{ highkey mf value; init() {{ self.value = 1; }} }} main_character {{ {body} }}"
            ),
            "LARP-T014",
        );
    }
    rejects(
        "main_character { nonchalant mf value = value; }",
        "LARP-N001",
    );
    rejects(
        "main_character { nonchalant mf value = 1; value = 2; }",
        "LARP-T008",
    );
    rejects(
        "main_character { nonchalant hediJeans<mf,1> value = [1]; value[0] = 2; }",
        "LARP-T008",
    );
}

#[test]
fn constructors_cannot_capture_partial_self_but_initialized_field_copies_can_be_deferred() {
    rejects(
        "archetype Box { mf value; init() { self.value = 1; nonchalant mf copied = self.value; } } main_character {}",
        "LARP-T012",
    );
    run(
        r#"
        archetype Box {
            highkey mf value;
            init() { self.value = 2; mf copied = self.value; nonchalant mf deferred = copied + 1; self.value = deferred; }
        }
        main_character { yap(new Box().value); }
    "#,
        "3\n",
        "",
    );
}

#[test]
fn scenario_local_lazy_cells_and_specialized_pure_helpers_remain_regional() {
    run(
        r#"
        archetype Counter { highkey mf value; init() { self.value = 0; } }
        sidequest helper(mf value) -> mf uses(abort) { nonchalant mf next = value + 1; return next; }
        main_character {
            let real = new Counter(); mf base = 3;
            larp (real) as Trial { nonchalant mf value = helper(base); real.value = value; canon real.value; }
            yap(real.value);
        }
    "#,
        "4\n",
        "",
    );
    rejects(
        "archetype Counter { highkey mf value; init() { self.value = 0; } } main_character { let real = new Counter(); nonchalant mf outer = 49; larp (real) as Trial { real.value = outer; } }",
        "LARP-S002",
    );
}

#[test]
fn nested_suppression_applies_to_direct_indirect_role_and_extension_calls() {
    run(
        r#"
        role Noisy { sidequest speak() -> unit uses(observe); }
        archetype Talker : Noisy { init() { sybau {} } highkey sidequest speak() -> unit uses(observe) { yap("role"); } }
        sidequest direct() -> unit uses(observe) { yap("direct"); }
        sidequest mf.speak() -> unit uses(observe) { yap(self); }
        main_character {
            Noisy talker = new Talker(); let callback = direct; mf number = 49;
            yap("before");
            sybau {
                direct(); callback(); talker.speak(); number.speak();
                mfs<mf> values = [1,2]; values.forEach(x => yap(x));
                sybau { yap("nested"); }
                yap("still muted");
            }
            yap("after");
        }
    "#,
        "before\nafter\n",
        "",
    );
}

#[test]
fn suppression_unwinds_on_return_and_keeps_an_inherited_outer_mute() {
    run(
        r#"
        sidequest early() -> mf uses(observe) { sybau { yap("hidden"); return 7; } }
        sidequest nested() -> unit uses(observe) { sybau { early(); } yap("after helper block"); }
        main_character {
            yap(early()); yap("visible");
            sybau { nested(); yap("caller still muted"); }
            yap("restored");
        }
    "#,
        "7\nvisible\nrestored\n",
        "",
    );
}

#[test]
fn breaks_unwind_only_suppression_scopes_crossed_by_the_exit() {
    run(
        r#"
        main_character {
            goon { sybau { yap("hidden exit"); touch_grass; } }
            yap("after outer break");
            sybau { goon { touch_grass; } yap("still muted"); }
            yap("after inner break");
            mf count = 0;
            locked_in while (count < 2) { sybau { count += 1; if (count == 2) { touch_grass; } } }
            yap(count);
        }
    "#,
        "after outer break\nafter inner break\n2\n",
        "",
    );
}

#[test]
fn suppression_does_not_erase_observation_effects_or_constructor_restrictions() {
    rejects(
        "sidequest pure() -> unit uses() { sybau { yap(1); } } main_character {}",
        "LARP-E001",
    );
    rejects(
        "archetype Box { init() { sybau { yap(1); } } } main_character {}",
        "LARP-E001",
    );
    rejects(
        "main_character { nonchalant mf value = (() => { sybau { yap(1); } return 1; })(); }",
        "LARP-E001",
    );
    run(
        "archetype Box { highkey mf value; init() { sybau { self.value = 49; } } } main_character { yap(new Box().value); }",
        "49\n",
        "",
    );
}

#[test]
fn scenario_inspection_remains_visible_inside_suppressed_blocks_and_calls() {
    run(
        r#"
        archetype Counter { highkey mf value; init() { self.value = 0; } }
        main_character {
            let real = new Counter();
            sybau { larp (real) as Trial { yap("hidden"); real.value = 49; canon real.value; palantir(real); } }
            yap(real.value);
        }
    "#,
        "49\n",
        "PALANTIR [Trial]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\nvalue | 0 | 49 | - | true\n",
    );
}

#[test]
fn fatal_failures_and_output_errors_are_not_hidden_by_suppression() {
    runtime_error(
        "main_character { sybau { yap(1); crashout(\"fatal\"); } }",
        "LARP-R003",
        "crashout(\"fatal\")",
        "",
    );
    runtime_error(
        "main_character { sybau { no_cap(cap); } }",
        "LARP-R003",
        "no_cap(cap)",
        "",
    );
    struct BrokenOutput;
    impl Write for BrokenOutput {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("sink failure"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let source = source("main_character { sybau { yap(1); } yap(2); }");
    let compilation = compile_source(&source);
    let diagnostic =
        larp_vm::execute(&compilation.bytecode, &mut BrokenOutput, Default::default()).unwrap_err();
    assert_eq!(diagnostic.code.id, "LARP-R007");
    assert_eq!(source.slice(diagnostic.primary.span), Some("yap(2)"));
}

#[test]
fn forcing_at_callback_capture_and_eager_fallbacks_keeps_abort_effects_visible() {
    rejects(
        "sidequest pure(mf base) -> fn()->mf uses() uses() { nonchalant mf value = base + 1; return () => value; } main_character {}",
        "LARP-E001",
    );
    rejects(
        "sidequest declaredAbort() -> mf uses(abort) { return 49; } sidequest pure() -> mf uses() { nonchalant mf value = declaredAbort(); return value; } main_character {}",
        "LARP-E001",
    );
    runtime_error(
        "main_character { mf zero = 0; nonchalant mf fatal = 1 / zero; baddie<mf> present = some(49); yap(present.copium(fatal)); }",
        "LARP-R002",
        "1 / zero",
        "",
    );
}

#[test]
fn lazy_initializers_remain_rooted_during_nested_closure_allocations_and_cell_churn() {
    let source = source(
        r#"
        sidequest force(mf value) -> mf uses(abort) {
            nonchalant mf pending = ((mf x) => x + 1)(value);
            return pending;
        }
        main_character {
            nonchalant mf live = ((mf x) => x + 1)(48);
            npc loop (mf i in 0..1000) { force(i); }
            yap(live);
            npc loop (mf i in 0..1000) { force(i); }
            yap(live);
        }
    "#,
    );
    let compilation = compile_source(&source);
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let stats = larp_vm::execute_with_stats(
            &compilation.bytecode,
            &mut output,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output, b"49\n49\n");
        assert_eq!(stats.allocations, stats.reclaimed);
        if gc_stress {
            assert!(stats.peak_live_objects < 20, "{stats:?}");
        }
    }
}

#[test]
fn suppression_preserves_errors_from_the_independent_inspection_sink() {
    struct BrokenInspection;
    impl Write for BrokenInspection {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("inspection failure"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let source = source(
        "archetype Counter { highkey mf value; init() { self.value = 0; } } main_character { let real = new Counter(); sybau { larp (real) as Trial { yap(1); palantir(real); } } }",
    );
    let compilation = compile_source(&source);
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let diagnostic = larp_vm::execute_with_observers(
            &compilation.bytecode,
            &mut output,
            &mut BrokenInspection,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(diagnostic.code.id, "LARP-R007");
        assert_eq!(
            source.slice(diagnostic.primary.span),
            Some("palantir(real)")
        );
        assert!(output.is_empty());
    }
}
