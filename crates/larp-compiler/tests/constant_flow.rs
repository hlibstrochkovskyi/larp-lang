use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "constant-flow.larp", text)
}

fn run(text: &str, expected: &str) {
    let compiled = compile(&source(text)).unwrap_or_else(|errors| panic!("{errors:#?}"));
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let stats = larp_vm::execute_with_stats(
            &compiled.bytecode,
            &mut output,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output, expected.as_bytes(), "gc_stress={gc_stress}");
        assert_eq!(stats.allocations, stats.reclaimed);
    }
}

fn rejects(text: &str, code: &str) {
    let file = source(text);
    let errors = compile(&file).err().expect("source must be rejected");
    assert!(
        errors.iter().any(|error| error.code.id == code),
        "{errors:#?}"
    );
    assert!(
        errors
            .iter()
            .all(|error| error.code.id != "LARP-B001" && file.slice(error.primary.span).is_some()),
        "{errors:#?}"
    );
}

#[test]
fn constant_conditions_select_returns_for_functions_methods_roles_extensions_and_lambdas() {
    run(
        r#"
        sidequest answer() -> mf uses() { if (fr) { return 42; } }
        sidequest choose() -> mf uses() {
            if (cap) { return 0; } else if (1 + 1 == 2 && !(3 < 2)) { return 43; }
        }
        sidequest mf.score() -> mf uses() { if (!cap) { return 49; } }
        sidequest quiet() -> mf uses(observe) {
            sybau { yap("muted"); if (fr) { return 46; } }
        }
        role Answers { sidequest answer() -> mf uses(); }
        archetype Solver : Answers {
            init() {}
            highkey sidequest answer() -> mf uses() { if (fr || cap) { return 42; } }
        }
        main_character {
            let inferred = () => { if (cap) { return 0; } if (fr) { return 44; } };
            fn() -> mf uses() contextual = () => { if (!cap) { return 45; } };
            Answers solver = new Solver();
            let bound = solver.answer;
            mf value = 0;
            yap(answer()); yap(choose()); yap(value.score());
            yap(inferred()); yap(contextual()); yap(bound()); yap(quiet());
        }
    "#,
        "42\n43\n49\n44\n45\n42\n46\n",
    );
}

#[test]
fn constant_loop_entries_preserve_returns_breaks_and_divergence() {
    run(
        r#"
        sidequest once() -> mf uses() { locked_in while (fr) { return 7; } }
        sidequest single_range() -> mf { npc loop (mf i in 0..1) { return 44; } }
        sidequest repeated() -> mf {
            mf count = 0;
            locked_in while (!cap) {
                if (cap) { touch_grass; }
                count += 1;
                if (count == 2) { return count; }
            }
        }
        main_character { yap(once()); yap(repeated()); yap(single_range()); }
    "#,
        "7\n2\n44\n",
    );
    let file = source(
        "sidequest forever() -> mf uses() { locked_in while (fr) {} } main_character { yap(forever()); }",
    );
    let compiled = compile(&file).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let mut output = Vec::new();
    let error = larp_vm::execute(
        &compiled.bytecode,
        &mut output,
        larp_vm::Limits {
            instructions: 100,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code.id, "LARP-R006");
    assert!(file.slice(error.primary.span).is_some());
    assert!(output.is_empty());
    for text in [
        "sidequest f() -> mf { if (cap) { return 1; } } main_character {}",
        "sidequest f(verdict flag) -> mf { if (flag) { return 1; } } main_character {}",
        "sidequest f() -> mf { locked_in while (cap) { return 1; } } main_character {}",
        "sidequest f(verdict flag) -> mf { locked_in while (fr) { if (flag) { touch_grass; } } } main_character {}",
        "sidequest f() -> mf { npc loop (mf i in 1..0) { return 44; } } main_character {}",
        "sidequest f(mf start, mf end) -> mf { npc loop (mf i in start..end) { return 44; } } main_character {}",
    ] {
        rejects(text, "LARP-T007");
    }
}

#[test]
fn unreachable_branches_still_check_names_types_effects_access_and_regions() {
    for (text, code) in [
        ("main_character { if (cap) { yap(missing); } }", "LARP-N001"),
        (
            "main_character { locked_in while (cap) { mf value = cap; } }",
            "LARP-T001",
        ),
        (
            "sidequest f() -> mf uses() { if (cap) { yap(1); } if (fr) { return 1; } } main_character {}",
            "LARP-E001",
        ),
        (
            "sidequest f() -> mf { if (fr) { return 1; } else { return cap; } } main_character {}",
            "LARP-T001",
        ),
        ("main_character { if (cap) { yap(1 / 0); } }", "LARP-T004"),
        (
            "archetype C { mf value; init() { self.value = 0; } } main_character { let c = new C(); if (cap) { yap(c.value); } }",
            "LARP-T010",
        ),
        (
            "archetype C { highkey mf value; init() { self.value = 0; } } main_character { let c = new C(); let alias = c; larp (c) as Trial { if (cap) { yap(alias.value); } } }",
            "LARP-S002",
        ),
        (
            "sidequest hidden() -> mf uses() { let unused = [1]; return 0; } archetype C { highkey mf value; init() { self.value = 0; } } main_character { let c = new C(); larp (c) as Trial { if (cap) { c.value = hidden(); } } }",
            "LARP-E002",
        ),
    ] {
        rejects(text, code);
    }
}

#[test]
fn settled_lambda_results_validate_earlier_dead_returns_without_forcing_divergent_results() {
    for body in [
        "if (cap) { return cap; } return 44;",
        "return 44; if (cap) { return cap; }",
        "if (fr) { return 44; } else { return cap; }",
    ] {
        rejects(
            &format!("main_character {{ let callback = () => {{ {body} }}; }}"),
            "LARP-T001",
        );
    }
    rejects(
        "main_character { fn() -> mf callback = () => { crashout(\"stop\"); return cap; }; }",
        "LARP-T001",
    );
    run(
        r#"
        main_character {
            let divergent = () => { locked_in while (fr) {} return cap; };
            let fatal = () => { crashout("stop"); return cap; };
            yap("ready");
        }
    "#,
        "ready\n",
    );
}

#[test]
fn constructor_initialization_tracks_feasible_literal_paths_and_short_circuit_reads() {
    run(
        r#"
        archetype Direct {
            highkey mf value;
            init() { if (fr) { self.value = 42; } }
        }
        archetype Early {
            highkey mf value;
            init() { if (fr) { self.value = 43; return; } }
        }
        archetype Looped {
            highkey mf value;
            init() { locked_in while (fr) { self.value = 44; touch_grass; } }
        }
        archetype Skipped {
            highkey mf value;
            init() {
                if (cap) { return; }
                verdict skipped = fr || (self.value == 0);
                self.value = 45;
            }
        }
        archetype EmptyRange {
            highkey mf value;
            init() {
                npc loop (mf i in 1..0) { self.value = self.value + 1; }
                self.value = 46;
            }
        }
        archetype NonemptyRange {
            highkey mf value;
            init() { npc loop (mf i in 0..1) { self.value = 47; } }
        }
        main_character {
            let a = new Direct(); let b = new Early(); let c = new Looped(); let d = new Skipped();
            let e = new EmptyRange(); let f = new NonemptyRange();
            yap(a.value); yap(b.value); yap(c.value); yap(d.value);
            yap(e.value); yap(f.value);
        }
    "#,
        "42\n43\n44\n45\n46\n47\n",
    );
    for text in [
        "archetype C { mf value; init() { if (cap) { self.value = 1; } } } main_character {}",
        "archetype C { mf value; init(verdict flag) { if (flag) { self.value = 1; } } } main_character {}",
        "archetype C { mf value; init() { locked_in while (cap) { self.value = 1; } } } main_character {}",
        "archetype C { mf value; init() { locked_in while (fr) { touch_grass; self.value = 1; } } } main_character {}",
        "archetype C { mf value; init() { npc loop (mf i in 1..0) { self.value = 1; } } } main_character {}",
        "archetype C { mf value; init(mf start, mf end) { npc loop (mf i in start..end) { self.value = 1; } } } main_character {}",
    ] {
        rejects(text, "LARP-T011");
    }
}
