use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn run(text: &str, expected: &str) {
    let source = SourceFile::new(SourceId(0), "callbacks.larp", text);
    let compilation = compile(&source).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(|error| larp_diagnostics::render(error, &source))
                .collect::<String>()
        )
    });
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
        assert_eq!(
            String::from_utf8(output).unwrap(),
            expected,
            "gc_stress={gc_stress}"
        );
        assert_eq!(
            stats.allocations, stats.reclaimed,
            "all runtime values are dead at program exit"
        );
    }
}

fn rejects(text: &str, code: &str) {
    let source = SourceFile::new(SourceId(0), "bad.larp", text);
    let errors = compile(&source).err().expect("must fail compilation");
    assert!(
        errors.iter().any(|error| error.code.id == code),
        "{errors:#?}"
    );
    assert!(
        errors.iter().all(|error| error.code.id != "LARP-B001"),
        "frontend errors must not reach invalid bytecode"
    );
}

#[test]
fn named_function_values_and_contextual_or_annotated_lambdas_are_callable() {
    run(
        r#"
        sidequest addOne(mf x) -> mf { return x + 1; }
        sidequest apply(fn(mf) -> mf callback, mf x) -> mf { return callback(x); }
        main_character {
            let named = addOne;
            yap(apply(named, 2));
            yap(apply(x => x * 2, 3));
            let annotated = (mf x) => x + 3;
            yap(annotated(4));
            let two = (mf x, mf y) => x * 10 + y;
            yap(two(2, 3));
            let nothing = () => { yap("unit"); };
            nothing();
        }
    "#,
        "3\n6\n7\n23\nunit\n",
    );
}

#[test]
fn prelude_functions_are_values_with_concrete_or_contextual_signatures() {
    run(
        r#"
        main_character {
            let parse = parseScore;
            let assert = no_cap;
            fn(mf) -> unit uses(observe) print = yap;
            fn(mf) -> baddie<mf> uses() wrap = some;
            fn(mf) -> result<mf, text> uses() success = win;
            fn(text) -> result<mf, text> uses() failure = fumble;
            fn(unc) -> result<mf, ConversionError> uses() convert = ascend;
            fn(unc) -> mf narrow = fraud;
            assert(parse("49").isOk());
            print(wrap(2).copium(0));
            print(success(3).cope(err => 0));
            print(failure("expected").cope(err => 4));
            print(convert(fortnine).cope(err => 0));
            print(narrow(fortnine));
        }
    "#,
        "2\n3\n4\n49\n49\n",
    );
    rejects("main_character { let print = yap; }", "LARP-T003");
    rejects(
        "main_character { fn(mf) -> unit uses() print = yap; }",
        "LARP-T001",
    );
}

#[test]
fn captures_are_snapshots_and_nested_returned_closures_survive_their_frames() {
    run(
        r#"
        sidequest nested(mf outer) -> fn(mf) -> fn() -> mf {
            return middle => () => outer + middle;
        }
        main_character {
            mf base = 10;
            let saved = (mf x) => base + x;
            base = 100;
            yap(saved(2));
            let shadow = () => { yap(base); mf base = base + 1; return base; };
            base = 200;
            yap(shadow());
            let middle = nested(20);
            let leaf = middle(3);
            yap(leaf());
        }
    "#,
        "12\n100\n101\n23\n",
    );
}

#[test]
fn function_captures_keep_original_code_and_callee_evaluation_precedes_arguments() {
    run(
        r#"
        sidequest maker() -> fn(mf) -> mf uses(abort) uses(observe) {
            yap(1);
            return x => x + 1;
        }
        sidequest argument() -> mf uses(observe) { yap(2); return 3; }
        main_character {
            let first = (mf x) => x + 1;
            let saved = (mf x) => first(x);
            first = (mf x) => x + 100;
            yap(saved(1));
            yap(maker()(argument()));
        }
    "#,
        "2\n1\n2\n4\n",
    );
}

#[test]
fn closures_cannot_mutate_captured_bindings_and_inference_is_local() {
    for (text, code) in [
        (
            "main_character { mf x = 1; let f = () => { x = 2; }; }",
            "LARP-T008",
        ),
        ("main_character { let f = x => x + 1; }", "LARP-T003"),
        ("main_character { let f = (mf x, mf x) => x; }", "LARP-N002"),
        (
            "main_character { let f = () => { touch_grass; }; }",
            "LARP-T009",
        ),
        (
            "main_character { let f = (verdict v) => { if (v) { return 1; } return cap; }; }",
            "LARP-T001",
        ),
        (
            "main_character { fn(mf) -> mf f = x => { if (x > 0) { return x; } }; }",
            "LARP-T007",
        ),
        (
            "main_character { let f = (mf x) => x; yap(f == f); }",
            "LARP-T001",
        ),
    ] {
        rejects(text, code);
    }
}

#[test]
fn effects_survive_function_variables_captures_and_higher_order_boundaries() {
    rejects(
        "sidequest hide(fn() -> mf uses(observe) callback) -> mf uses() { let saved = callback; return saved(); } main_character {}",
        "LARP-E001",
    );
    rejects(
        "main_character { fn() -> mf uses() f = () => { yap(1); return 0; }; }",
        "LARP-E001",
    );
    rejects(
        "sidequest noisy() -> mf uses(observe) { yap(1); return 0; } main_character { fn() -> mf uses() f = noisy; }",
        "LARP-T001",
    );
    rejects(
        "sidequest pure() -> mf uses() { fn() -> mf uses(observe) f = () => 1; return f(); } main_character {}",
        "LARP-E001",
    );
    run(
        "sidequest apply(fn(mf) -> mf uses() f) -> mf uses() { return f(1); } main_character { yap(apply(x => 7)); }",
        "7\n",
    );
}

#[test]
fn options_are_explicit_nested_value_types_and_copium_evaluates_fallback_eagerly() {
    run(
        r#"
        sidequest fallback() -> unc uses(observe) { yap("fallback"); return 0; }
        main_character {
            baddie<unc> none = no_bitches;
            baddie<unc> someValue = some(49);
            no_cap(!none.isSome()); no_cap(someValue.isSome());
            yap(none.copium(20)); yap(someValue.copium(fallback()));
            baddie<baddie<mf>> nested = some(some(3));
            yap(nested.copium(no_bitches).copium(0));
            no_cap(someValue == some(49));
            baddie<unc> another = no_bitches;
            no_cap(none == another);
        }
    "#,
        "20\nfallback\n49\n3\n",
    );
    rejects("main_character { let none = no_bitches; }", "LARP-T003");
    rejects("main_character { baddie<mf> x = 1; }", "LARP-T001");
    rejects(
        "main_character { baddie<mf> x = some(1); unc y = x.copium(0); }",
        "LARP-T001",
    );
}

#[test]
fn results_recover_only_errors_but_create_handlers_eagerly() {
    run(
        r#"
        sidequest handler() -> fn(text) -> mf uses() uses(observe) {
            yap("created"); return err => 99;
        }
        main_character {
            result<mf, text> good = win(7);
            result<mf, text> bad = fumble("bad");
            no_cap(good.isOk()); no_cap(!bad.isOk());
            yap(good.cope(handler()));
            yap(good.cope(err => { crashout("must not run"); return 0; }));
            yap(bad.cope(err => { yap(err); return 0; }));
            result<mf, text> same = win(7);
            no_cap(good == same); no_cap(good != bad);
        }
    "#,
        "created\n7\n7\nbad\n0\n",
    );
    rejects("main_character { let x = win(1); }", "LARP-T003");
    rejects("main_character { let x = fumble(\"bad\"); }", "LARP-T003");
    rejects(
        "main_character { result<mf, text> x = win(1); mf y = x.cope(err => cap); }",
        "LARP-T001",
    );
}

#[test]
fn recovery_can_be_pure_and_function_payloads_remain_rooted() {
    run(
        r#"
        sidequest parse(text input) -> mf uses() { return parseScore(input).cope(err => 0); }
        main_character {
            yap(parse("49")); yap(parse("bad"));
            baddie<fn(mf) -> mf> optional = some((mf x) => x + 1);
            let selected = optional.copium((mf x) => x + 100);
            yap(selected(3));
            result<fn(mf) -> mf, text> saved = win((mf x) => x * 2);
            let recovered = saved.cope(err => (mf x) => x + 1);
            yap(recovered(4));
        }
    "#,
        "49\n0\n4\n8\n",
    );
}

#[test]
fn parse_score_validates_the_entire_signed_decimal_string() {
    run(
        r#"
        main_character {
            yap(parseScore("-2147483648").cope(err => 0));
            yap(parseScore("2147483647").cope(err => 0));
            no_cap(!parseScore("2147483648").isOk());
            no_cap(!parseScore("+1").isOk());
            no_cap(!parseScore(" 1").isOk());
            no_cap(!parseScore("1_0").isOk());
            no_cap(!parseScore("-").isOk());
            no_cap(!parseScore("").isOk());
            no_cap(!parseScore("１").isOk());
        }
    "#,
        "-2147483648\n2147483647\n",
    );
}

#[test]
fn checked_conversions_return_nominal_error_values_or_trap_explicitly() {
    run(
        r#"
        main_character {
            unc large = 3_000_000_000;
            result<mf, ConversionError> converted = ascend<mf>(large);
            no_cap(!converted.isOk());
            yap(converted.cope(err => { yap(err.message); return 49; }));
            yap(ascend<unc>(49).cope(err => 0));
            unc small = 20;
            yap(fraud<mf>(small));
            yap(ascend<unc>(small).cope(err => 0));
        }
    "#,
        "value does not fit mf\n49\n49\n20\n20\n",
    );
    rejects("main_character { let x = ascend<geeked>(1); }", "LARP-T001");
    rejects("main_character { let x = ascend<mf>(1.0); }", "LARP-T001");
    let source = SourceFile::new(
        SourceId(0),
        "trap.larp",
        "main_character { unc large = 3_000_000_000; yap(fraud<mf>(large)); }",
    );
    let compilation = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let diagnostic =
        larp_vm::execute(&compilation.bytecode, &mut Vec::new(), Default::default()).unwrap_err();
    assert_eq!(diagnostic.code.id, "LARP-R008");
    assert_eq!(
        source.slice(diagnostic.primary.span),
        Some("fraud<mf>(large)")
    );
}

#[test]
fn closure_churn_is_reclaimed_during_execution_and_live_chains_are_not_lost() {
    let source = SourceFile::new(
        SourceId(0),
        "gc.larp",
        r#"
        sidequest make(mf x) -> fn() -> mf uses() uses() { return () => x; }
        main_character {
            let live = make(49);
            npc loop (mf i in 0..2000) { let garbage = make(i); }
            yap(live());
        }
    "#,
    );
    let compilation = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let mut output = Vec::new();
    let stats = larp_vm::execute_with_stats(
        &compilation.bytecode,
        &mut output,
        larp_vm::Limits {
            gc_stress: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(output, b"49\n");
    assert_eq!(stats.allocations, 2001);
    assert!(stats.collections >= 2001);
    assert!(stats.peak_live_objects < 20);
    assert_eq!(stats.reclaimed, stats.allocations);
}

#[test]
fn nonreturning_calls_do_not_constrain_inferred_results_and_accept_condition_context() {
    run(
        r#"
        main_character {
            verdict stop = cap;
            let callback = () => {
                if (stop) { return crashout("stop"); }
                return 49;
            };
            yap(callback());
            let dead = () => { crashout("stop"); return cap; };
            no_cap(fr || crashout("skipped"));
        }
    "#,
        "49\n",
    );
    for text in [
        "main_character { if (crashout(\"stop\")) { yap(1); } }",
        "main_character { locked_in while (crashout(\"stop\")) { yap(1); } }",
    ] {
        let source = SourceFile::new(SourceId(0), "never.larp", text);
        let compilation = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
        let mut output = Vec::new();
        let diagnostic =
            larp_vm::execute(&compilation.bytecode, &mut output, Default::default()).unwrap_err();
        assert_eq!(diagnostic.code.id, "LARP-R003");
        assert_eq!(
            source.slice(diagnostic.primary.span),
            Some("crashout(\"stop\")")
        );
        assert!(output.is_empty());
    }
    rejects(
        "main_character { let f = () => { crashout(\"stop\"); let bad = unknown; }; }",
        "LARP-N001",
    );
    rejects("main_character { if (49) {} }", "LARP-T002");
}
