use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "scenarios.larp", text)
}

fn run(text: &str, expected: &str, expected_inspection: &str) {
    let source = source(text);
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

fn rejects(text: &str, expected: &str) {
    let source = source(text);
    let errors = compile(&source)
        .err()
        .expect("Expected a scenario rejection");
    assert!(
        errors.iter().any(|error| error.code.id == expected),
        "{}",
        errors
            .iter()
            .map(|error| larp_diagnostics::render(error, &source))
            .collect::<String>()
    );
    assert!(
        errors.iter().all(|error| error.code.id != "LARP-B001"),
        "Source scenario errors must be diagnosed before bytecode verification"
    );
    assert!(
        errors
            .iter()
            .all(|error| source.slice(error.primary.span).is_some())
    );
}

const COUNTER: &str = r#"
    archetype Counter {
        highkey mf value;
        init() { self.value = 0; }
        highkey sidequest add(mf amount) -> unit uses(read,write,abort) { self.value += amount; }
        highkey sidequest get() -> mf uses(read) { return self.value; }
    }
"#;

#[test]
fn existing_scenario_programs_execute_through_verified_bytecode() {
    run(
        include_str!("../../../tests/conformance/pass/scenario.larp"),
        "20\n3\n",
        "PALANTIR [PeterThiel]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\nvaluation | 20 | 1000000000 | 1000000000 | false\naura | 0 | 3 | - | true\n",
    );
    run(
        include_str!("../../../tests/conformance/pass/scenario-method.larp"),
        "1\n",
        "",
    );
    run(
        include_str!("../../../tests/conformance/pass/scenario-result-is-data.larp"),
        "7\n",
        "",
    );
}

#[test]
fn existing_scenario_rejections_have_frontend_codes() {
    for (text, code) in [
        (
            include_str!("../../../tests/conformance/fail/scenario-alias.larp"),
            "LARP-S002",
        ),
        (
            include_str!("../../../tests/conformance/fail/scenario-escape.larp"),
            "LARP-S004",
        ),
        (
            include_str!("../../../tests/conformance/fail/scenario-nested.larp"),
            "LARP-S003",
        ),
        (
            include_str!("../../../tests/conformance/fail/scenario-return.larp"),
            "LARP-S005",
        ),
        (
            include_str!("../../../tests/conformance/fail/scenario-reference-field.larp"),
            "LARP-S001",
        ),
        (
            include_str!("../../../tests/conformance/fail/scenario-hidden-allocation.larp"),
            "LARP-E002",
        ),
        (
            include_str!("../../../tests/conformance/fail/canon-outside.larp"),
            "LARP-S006",
        ),
    ] {
        rejects(text, code);
    }
}

#[test]
fn selection_commits_final_values_and_claims_survive_ordinary_writes() {
    run(
        r#"
        archetype State {
            highkey mf a;
            highkey mf b;
            init() { self.a = 1; self.b = 2; }
        }
        main_character {
            let state = new State(); let alias = state;
            larp (state) as Trial {
                canon state.a; canon state.a;
                flex state.a = 10;
                state.a = 11;
                flex state.b = 20;
                state.b = 21;
                palantir(state);
            }
            yap(alias.a); yap(alias.b);
        }
    "#,
        "11\n2\n",
        "PALANTIR [Trial]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\na | 1 | 11 | 10 | true\nb | 2 | 21 | 20 | false\n",
    );
}

#[test]
fn conditional_selection_and_sequential_regions_use_each_entry_baseline() {
    run(
        &format!(
            r#"{COUNTER}
        main_character {{
            let real = new Counter();
            verdict accept = cap;
            larp (real) as Discard {{ real.value = 10; if (accept) {{ canon real.value; }} }}
            yap(real.value);
            accept = fr;
            larp (real) as First {{ real.value = 20; if (accept) {{ canon real.value; }} }}
            larp (real) as Second {{ real.value += 2; canon real.value; palantir(real); }}
            yap(real.value);
        }}
    "#
        ),
        "0\n22\n",
        "PALANTIR [Second]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\nvalue | 20 | 22 | - | true\n",
    );
}

#[test]
fn methods_helpers_recursion_and_extensions_route_every_root_access_to_performed_state() {
    run(
        r#"
        archetype Counter {
            highkey mf value;
            init() { self.value = 0; }
            sidequest privateAdd(mf amount) -> unit uses(read,write,abort) { self.value += amount; }
            highkey sidequest add(mf amount) -> unit uses(read,write,abort) { self.privateAdd(amount); }
            highkey sidequest read() -> mf uses(read) { return self.value; }
        }
        sidequest forward(Counter root, mf count) -> unit uses(read,write,abort) { recurse(root, count); }
        sidequest recurse(Counter root, mf count) -> unit uses(read,write,abort) {
            if (count > 0) { root.add(2); forward(root, count - 1); }
        }
        sidequest Counter.bonus() -> unit uses(read,write,abort) { self.add(1); }
        main_character {
            let real = new Counter();
            larp (real) as Trial { forward(real, 3); real.bonus(); yap(real.read()); canon real.value; }
            yap(real.value);
            real.add(3); yap(real.value);
        }
    "#,
        "7\n7\n10\n",
        "",
    );
}

#[test]
fn snapshot_imports_are_immutable_entry_copies_with_ordinary_inner_shadowing() {
    run(
        &format!(
            r#"{COUNTER}
        main_character {{
            let real = new Counter(); mf amount = 3;
            hediJeans<mf,2> row = [4,5]; baddie<mf> optional = some(6);
            larp (real) as Trial {{
                mf first = amount;
                mf amount = amount + 1;
                hediJeans<mf,2> local = row; local[0] = 49;
                real.value = first + amount + row[0] + optional.copium(0);
                canon real.value;
            }}
            yap(real.value); yap(amount); yap(row[0]);
        }}
    "#
        ),
        "17\n3\n4\n",
        "",
    );
    for statement in ["amount = 4;", "amount += 1;", "row[0] = 1;"] {
        rejects(
            &format!(
                "{COUNTER} main_character {{ let real = new Counter(); mf amount = 3; hediJeans<mf,1> row = [0]; larp (real) as Trial {{ {statement} }} }}"
            ),
            "LARP-S004",
        );
    }
}

#[test]
fn reference_aliases_and_outer_function_bindings_cannot_be_read_or_captured() {
    for statement in [
        "yap(alias.value);",
        "alias.add(1);",
        "let copied = alias;",
        "let callback = () => alias.value;",
        "callback();",
        "let copied = callback;",
        "let nested = () => callback();",
        "yap(values.size());",
    ] {
        rejects(
            &format!(
                "{COUNTER} main_character {{ let real = new Counter(); let alias = real; let callback = () => 1; mfs<mf> values = []; larp (real) as Trial {{ {statement} }} }}"
            ),
            "LARP-S002",
        );
    }
    run(
        &format!(
            "{COUNTER} main_character {{ let real = new Counter(); let alias = real; larp (real) as Trial {{ mf alias = 7; real.value = alias; canon real.value; }} yap(real.value); }}"
        ),
        "7\n",
        "",
    );
}

#[test]
fn local_value_callbacks_and_snapshot_captures_are_safe_under_indirect_calls() {
    run(
        &format!(
            r#"{COUNTER}
        sidequest apply(fn(mf)->mf uses(abort) callback, mf value) -> mf uses(abort) {{ return callback(value); }}
        sidequest increment(mf value) -> mf uses(abort) {{ return value + 1; }}
        main_character {{
            let real = new Counter(); mf offset = 3;
            larp (real) as Trial {{
                let callback = (mf value) => offset + value;
                let named = increment;
                real.value = apply(callback, named(4));
                result<mf,text> failed = fumble("expected");
                real.value += failed.cope(err => offset);
                mf observed = real.value; let observe = () => {{ yap(observed); }}; observe();
                canon real.value;
            }}
            yap(real.value);
        }}
    "#
        ),
        "11\n11\n",
        "",
    );
}

#[test]
fn regional_values_and_views_cannot_escape_through_bindings_bound_methods_or_identity() {
    for (statement, code) in [
        ("let copied = real;", "LARP-S004"),
        ("let bound = real.get;", "LARP-S004"),
        ("no_cap(real == real);", "LARP-S004"),
        ("real = new Counter();", "LARP-S004"),
        ("outside = real.get();", "LARP-S004"),
        ("saved = () => 7;", "LARP-S004"),
    ] {
        rejects(
            &format!(
                "{COUNTER} main_character {{ let real = new Counter(); mf outside = 0; let saved = () => 1; larp (real) as Trial {{ {statement} }} }}"
            ),
            code,
        );
    }
    rejects(
        &format!(
            "{COUNTER} sidequest leak(Counter value) -> Counter uses() {{ return value; }} main_character {{ let real = new Counter(); larp (real) as Trial {{ leak(real); }} }}"
        ),
        "LARP-E002",
    );
}

#[test]
fn region_boundaries_reject_crossing_exits_but_local_loops_and_callback_returns_work() {
    run(
        &format!(
            r#"{COUNTER}
        main_character {{
            let real = new Counter();
            larp (real) as Trial {{
                goon {{ real.value += 1; touch_grass; }}
                let callback = () => {{ return 2; }};
                real.value += callback();
                canon real.value;
            }}
            yap(real.value);
        }}
    "#
        ),
        "3\n",
        "",
    );
    for statement in ["return;", "touch_grass;"] {
        rejects(
            &format!(
                "{COUNTER} main_character {{ let real = new Counter(); goon {{ larp (real) as Trial {{ {statement} }} touch_grass; }} }}"
            ),
            "LARP-S005",
        );
    }
    rejects(
        &format!(
            "{COUNTER} main_character {{ let real = new Counter(); larp (real) as Outer {{ larp (real) as Inner {{}} }} }}"
        ),
        "LARP-S003",
    );
}

#[test]
fn root_layouts_are_snapshot_only_and_role_roots_are_rejected() {
    for field in [
        "mfs<mf>",
        "baddie<Counter>",
        "fn()->mf uses()",
        "hediJeans<mfs<mf>,0>",
    ] {
        let initializer = match field {
            "mfs<mf>" | "hediJeans<mfs<mf>,0>" => "[]",
            "baddie<Counter>" => "no_bitches",
            _ => "() => 1",
        };
        rejects(
            &format!(
                "{COUNTER} archetype Bad {{ highkey {field} value; init() {{ self.value = {initializer}; }} }} main_character {{ let bad = new Bad(); larp (bad) as Trial {{}} }}"
            ),
            "LARP-S001",
        );
    }
    rejects(
        "role R {} archetype A : R { init() {} } main_character { R value = new A(); larp (value) as Trial {} }",
        "LARP-S001",
    );
    rejects(
        "main_character { mf value = 0; larp (value) as Trial {} }",
        "LARP-S001",
    );
}

#[test]
fn transitive_call_summaries_reject_reference_allocation_dispatch_and_captures() {
    for helper in [
        "sidequest hidden() -> mf uses() { mfs<mf> unused = [1]; return 0; }",
        "sidequest hidden() -> mf uses(read,write,abort) { let unused = new Counter(); return 0; }",
        "sidequest hidden() -> mf uses(abort) { return deeper(); } sidequest deeper() -> mf uses() { mfs<mf> unused = []; return 0; }",
    ] {
        rejects(
            &format!(
                "{COUNTER} {helper} main_character {{ let real = new Counter(); larp (real) as Trial {{ real.value = hidden(); }} }}"
            ),
            "LARP-E002",
        );
    }
    rejects(
        &format!(
            "{COUNTER} sidequest twice(Counter a, Counter b) -> unit uses() {{}} main_character {{ let real = new Counter(); larp (real) as Trial {{ twice(real, real); }} }}"
        ),
        "LARP-E002",
    );
    rejects(
        &format!(
            "{COUNTER} main_character {{ let real = new Counter(); larp (real) as Trial {{ let captured = () => real.value; }} }}"
        ),
        "LARP-E002",
    );
}

#[test]
fn fixed_option_result_and_conversion_error_fields_commit_by_value() {
    run(
        r#"
        archetype Data {
            highkey hediJeans<mf,2> row;
            highkey baddie<mf> optional;
            highkey result<mf,text> parsed;
            highkey ConversionError error;
            init(ConversionError error) {
                self.row = [1,2]; self.optional = no_bitches; self.parsed = win(3);
                self.error = error;
            }
        }
        main_character {
            unc large = 3_000_000_000;
            mfs<ConversionError> errors = [];
            ascend<mf>(large).cope(err => { errors.push(err); return 0; });
            let real = new Data(errors[0]);
            larp (real) as DataTrial {
                real.row[1] = 9; canon real.row;
                real.optional = some(7); canon real.optional;
                real.parsed = fumble("expected");
                flex real.error = real.error; canon real.error;
            }
            yap(real.row[0]); yap(real.row[1]); yap(real.optional.copium(0));
            yap(real.parsed.cope(err => 99)); yap(real.error.message);
        }
    "#,
        "1\n9\n7\n3\nvalue does not fit mf\n",
        "",
    );
}

#[test]
fn private_visibility_controls_root_writes_claims_selection_and_inspection() {
    run(
        r#"
        archetype State {
            highkey mf visible;
            mf hidden;
            init() { self.visible = 1; self.hidden = 2; }
        }
        main_character { let real = new State(); larp (real) as Public { real.visible = 3; palantir(real); } }
    "#,
        "",
        "PALANTIR [Public]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\nvisible | 1 | 3 | - | false\n",
    );
    for statement in [
        "yap(real.hidden);",
        "real.hidden = 3;",
        "flex real.hidden = 3;",
        "canon real.hidden;",
    ] {
        rejects(
            &format!(
                "archetype State {{ mf hidden; init() {{ self.hidden = 2; }} }} main_character {{ let real = new State(); larp (real) as Trial {{ {statement} }} }}"
            ),
            "LARP-T010",
        );
    }
}

#[test]
fn scenario_operations_require_the_active_root_and_observer_names_can_be_shadowed() {
    for statement in [
        "canon real.value;",
        "flex real.value = 1;",
        "palantir(real);",
    ] {
        rejects(
            &format!("{COUNTER} main_character {{ let real = new Counter(); {statement} }}"),
            "LARP-S006",
        );
    }
    rejects(
        &format!(
            "{COUNTER} main_character {{ let real = new Counter(); let other = real; larp (real) as Trial {{ canon other.value; }} }}"
        ),
        "LARP-S006",
    );
    run(
        "sidequest palantir(mf value) -> unit uses(observe) { yap(value); } main_character { palantir(49); }",
        "49\n",
        "",
    );
}

#[test]
fn abort_keeps_emitted_output_and_reports_the_original_source_span() {
    let text = format!(
        "{COUNTER} main_character {{ let real = new Counter(); larp (real) as Trial {{ real.value = 49; canon real.value; yap(7); palantir(real); crashout(\"stop\"); }} }}"
    );
    let source = source(&text);
    let compilation = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let mut inspection = Vec::new();
        let diagnostic = larp_vm::execute_with_observers(
            &compilation.bytecode,
            &mut output,
            &mut inspection,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(diagnostic.code.id, "LARP-R003");
        assert_eq!(
            source.slice(diagnostic.primary.span),
            Some("crashout(\"stop\")")
        );
        assert_eq!(output, b"7\n");
        assert_eq!(inspection, b"PALANTIR [Trial]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\nvalue | 0 | 49 | - | true\n");
    }
}

#[test]
fn selective_commit_preserves_field_types_but_can_break_cross_field_invariants() {
    run(
        r#"
        archetype Account {
            highkey mf balance;
            highkey mf reserved;
            init() { self.balance = 100; self.reserved = 20; }
        }
        main_character {
            let account = new Account();
            no_cap(account.balance >= account.reserved);
            larp (account) as Counterexample {
                account.balance = 10;
                account.reserved = 0;
                no_cap(account.balance >= account.reserved);
                canon account.balance;
            }
            yap(account.balance); yap(account.reserved);
            yap(account.balance >= account.reserved);
        }
    "#,
        "10\n20\ncap\n",
        "",
    );
}

#[test]
fn root_helpers_accept_the_view_in_any_parameter_slot_and_retain_private_method_origins() {
    run(
        r#"
        archetype Counter {
            mf hidden;
            init() { self.hidden = 0; }
            sidequest addPrivate(mf value) -> unit uses(read,write,abort) { self.hidden += value; }
            highkey sidequest add(mf value) -> unit uses(read,write,abort) { self.addPrivate(value); }
            highkey sidequest read() -> mf uses(read) { return self.hidden; }
            highkey sidequest simulate() -> unit uses(read,write,abort) {
                let root = self;
                larp (root) as Private {
                    forward(3, root, 2);
                    canon root.hidden;
                }
            }
        }
        sidequest forward(mf count, Counter value, mf amount) -> unit uses(read,write,abort) {
            if (count > 0) { value.add(amount); forward(count - 1, value, amount); }
        }
        main_character { let real = new Counter(); real.simulate(); yap(real.read()); }
    "#,
        "6\n",
        "",
    );
}

#[test]
fn inspection_inside_a_declaring_class_reports_private_fields_without_exposing_outer_self() {
    run(
        r#"
        archetype State {
            highkey mf visible;
            mf hidden;
            init() { self.visible = 1; self.hidden = 2; }
            highkey sidequest simulate() -> unit uses(read,write,abort,observe) {
                let root = self;
                larp (root) as Owned {
                    flex root.hidden = 9;
                    canon root.hidden;
                    palantir(root);
                }
            }
            highkey sidequest read() -> mf uses(read) { return self.hidden; }
        }
        main_character { let real = new State(); real.simulate(); yap(real.read()); }
    "#,
        "9\n",
        "PALANTIR [Owned]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\nvisible | 1 | 1 | - | false\nhidden | 2 | 9 | 9 | true\n",
    );
    rejects(
        "archetype State { highkey mf value; init() { self.value = 0; } highkey sidequest simulate(State other) -> unit uses(read,write,abort,observe) { larp (other) as Trial { yap(self.value); } } } main_character {}",
        "LARP-S002",
    );
}

#[test]
fn observer_references_and_transitively_nested_scenarios_have_source_diagnostics() {
    rejects("main_character { let observer = palantir; }", "LARP-S006");
    rejects(
        &format!(
            "{COUNTER} main_character {{ let real = new Counter(); larp (real) as Trial {{ let observer = palantir; }} }}"
        ),
        "LARP-S006",
    );
    rejects(
        &format!(
            "{COUNTER} sidequest nested(Counter value) -> unit uses(read,write,abort) {{ larp (value) as Inner {{ value.value = 1; }} }} main_character {{ let real = new Counter(); larp (real) as Outer {{ nested(real); }} }}"
        ),
        "LARP-E002",
    );
    rejects(
        &format!(
            "{COUNTER} sidequest hidden() -> mf uses(io) {{ return 1; }} main_character {{ let real = new Counter(); larp (real) as Trial {{ real.value = hidden(); }} }}"
        ),
        "LARP-E002",
    );
}

#[test]
fn regional_specialization_rejects_role_dispatch_hidden_in_a_transitive_helper() {
    rejects(
        r#"
        role Readable { sidequest read() -> mf uses(read); }
        archetype Counter : Readable {
            highkey mf value;
            init() { self.value = 0; }
            highkey sidequest read() -> mf uses(read) { return self.value; }
        }
        sidequest hidden(Counter value) -> mf uses(read) { Readable roleValue = value; return roleValue.read(); }
        main_character { let real = new Counter(); larp (real) as Trial { real.value = hidden(real); } }
    "#,
        "LARP-E002",
    );
}

#[test]
fn excessive_snapshot_import_signatures_fail_before_bytecode_verification() {
    let mut text = format!("{COUNTER} main_character {{ let real = new Counter();");
    for index in 0..4096 {
        text.push_str(&format!("mf imported_{index} = 0;"));
    }
    text.push_str("larp (real) as TooManyImports {} }");
    rejects(&text, "LARP-P003");
}

#[test]
fn recursive_regional_call_graphs_are_specialized_once_and_execute_iteratively() {
    let text = format!(
        r#"{COUNTER}
        sidequest first(mf count, Counter root) -> unit uses(read,write,abort) {{
            if (count > 0) {{ root.add(1); second(count - 1, root); }}
        }}
        sidequest second(mf count, Counter root) -> unit uses(read,write,abort) {{ first(count, root); }}
        main_character {{ let real = new Counter(); larp (real) as Recursive {{ first(300, real); canon real.value; }} yap(real.value); }}
    "#
    );
    let compilation = compile(&source(&text)).unwrap_or_else(|errors| panic!("{errors:#?}"));
    assert!(compilation.bytecode.module().functions.len() < 20);
    run(&text, "300\n", "");
}
