use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "objects.larp", text)
}
fn run(text: &str, expected: &str) {
    let source = source(text);
    let compiled = compile(&source).unwrap_or_else(|errors| {
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
    let errors = compile(&source(text)).err().expect("must reject");
    assert!(
        errors.iter().any(|error| error.code.id == code),
        "{text}\n{errors:#?}"
    );
    assert!(
        errors.iter().all(|error| error.code.id != "LARP-B001"),
        "frontend failure reached verifier: {errors:#?}"
    );
}

#[test]
fn nominal_classes_preserve_identity_and_shared_field_mutation() {
    run(
        r#"
        archetype Founder {
            highkey text name;
            highkey unc netWorth;
            lowkey mf aura;
            init(text name, unc money) { self.name = name; self.netWorth = money; self.aura = 0; }
            highkey sidequest isMogging(mf opponent) -> verdict uses(read) { return self.aura mogg opponent; }
            highkey sidequest train(mf bonus) -> unit uses(read, write, abort) { self.aura += bonus; }
            highkey sidequest score() -> mf uses(read) { return self.aura; }
        }
        main_character {
            Founder bro = new Founder("unc", 20);
            let alias = bro;
            alias.train(5);
            no_cap(bro == alias);
            let another = new Founder("unc", 20);
            no_cap(another != bro);
            yap(bro.name); yap(bro.netWorth); yap(bro.isMogging(2)); yap(bro.score());
        }
    "#,
        "unc\n20\nfr\n5\n",
    );
}

#[test]
fn methods_and_nested_closures_can_intentionally_export_private_capabilities() {
    run(
        r#"
        archetype Counter {
            mf value;
            init(mf value) { self.value = value; }
            sidequest read() -> mf uses(read) { return self.value; }
            highkey sidequest bound() -> fn() -> mf uses(read) uses() { return self.read; }
            highkey sidequest nested() -> fn() -> fn() -> mf uses(read) uses() uses() { return () => () => self.value; }
            highkey sidequest add(mf amount) -> unit uses(read, write, abort) { self.value += amount; }
        }
        main_character {
            let counter = new Counter(1);
            let bound = counter.bound();
            let make = counter.nested();
            let leaf = make();
            counter.add(4);
            yap(bound()); yap(leaf());
        }
    "#,
        "5\n5\n",
    );
}

#[test]
fn bound_methods_save_the_original_receiver_and_evaluate_callee_before_arguments() {
    run(
        r#"
        archetype Counter {
            highkey mf value;
            init(mf value) { self.value = value; }
            highkey sidequest add(mf amount) -> mf uses(read, write, abort) { self.value += amount; return self.value; }
        }
        sidequest receiver() -> Counter uses(observe, read, write, abort) { yap("receiver"); return new Counter(10); }
        sidequest argument() -> mf uses(observe) { yap("argument"); return 2; }
        main_character {
            let original = new Counter(1);
            let alias = original;
            let bound = original.add;
            original = new Counter(100);
            yap(bound(3)); yap(alias.value); yap(original.value);
            yap(receiver().add(argument()));
        }
    "#,
        "4\n4\n100\nreceiver\nargument\n12\n",
    );
    run(
        r#"
        archetype Holder {
            highkey fn(mf) -> mf uses() callback;
            init() { self.callback = x => x; }
        }
        sidequest replace(Holder holder) -> mf uses(write) {
            holder.callback = x => 49;
            return 1;
        }
        main_character {
            let holder = new Holder();
            yap(holder.callback(replace(holder)));
            yap(holder.callback(0));
        }
    "#,
        "1\n49\n",
    );
}

#[test]
fn constructors_prove_initialization_across_branches_returns_and_loop_breaks() {
    run(
        r#"
        archetype Choice {
            highkey mf first;
            highkey mf second;
            init(verdict early) {
                if (early) { self.first = 1; self.second = self.first; return; }
                self.first = 2;
                goon { self.second = self.first + 1; touch_grass; }
            }
        }
        main_character { yap(new Choice(fr).second); yap(new Choice(cap).second); }
    "#,
        "1\n3\n",
    );
    for body in [
        "",
        "self.first = self.first; self.second = 0;",
        "if (condition) { self.first = 1; } self.second = 2;",
        "self.first = 1; if (condition) { return; } self.second = 2;",
        "locked_in while (condition) { self.first = 1; } self.second = 2;",
        "self.first = 1; let read = self.second; self.second = 2;",
    ] {
        rejects(
            &format!(
                "archetype Bad {{ mf first; mf second; init(verdict condition) {{ {body} }} }} main_character {{}}"
            ),
            "LARP-T011",
        );
    }
}

#[test]
fn constructors_create_fresh_nested_objects_and_can_store_deferred_mutating_callbacks() {
    run(
        r#"
        archetype Child { highkey mf value; init(mf value) { self.value = value; } }
        archetype Parent {
            highkey Child child;
            highkey fn() -> unit uses(write, abort) run;
            fn() -> mf uses() calculate;
            highkey mf value;
            init(mfs<mf> target) {
                self.child = new Child(7);
                self.run = () => target.push(49);
                self.calculate = () => 20;
                self.value = self.calculate();
            }
        }
        main_character {
            mfs<mf> target = [];
            let parent = new Parent(target);
            parent.run();
            yap(parent.child.value); yap(parent.value); yap(target[0]);
        }
    "#,
        "7\n20\n49\n",
    );
}

#[test]
fn partial_self_cannot_escape_be_captured_call_methods_or_return_existing_identity() {
    for body in [
        "let escaped = self;",
        "self = new Bad();",
        "return self;",
        "return new Bad();",
        "self.method();",
        "let bound = self.method;",
        "let capture = () => self.value;",
        "let nested = () => () => self.value;",
        "let values = [self];",
    ] {
        rejects(
            &format!(
                "archetype Bad {{ mf value; init() {{ self.value = 1; {body} }} highkey sidequest method() -> unit uses() {{}} }} main_character {{}}"
            ),
            "LARP-T012",
        );
    }
}

#[test]
fn constructors_reject_external_writes_including_aliases_and_indirect_callbacks() {
    let prefix = "archetype Box { highkey mf value; init() { self.value = 0; } }";
    for body in [
        "other.value = 1;",
        "values.push(1);",
        "values[0] = 1;",
        "let mutate = () => values.push(1); mutate();",
        "callback();",
        "mutate(values);",
    ] {
        rejects(
            &format!(
                "{prefix} sidequest mutate(mfs<mf> values) -> unit uses(write, abort) {{ values.push(1); }} archetype Bad {{ init(Box other, mfs<mf> values, fn() -> unit uses(write) callback) {{ {body} }} }} main_character {{}}"
            ),
            "LARP-T012",
        );
    }
    rejects(
        "archetype Bad { init() { yap(1); } } main_character {}",
        "LARP-E001",
    );
}

#[test]
fn instance_self_is_read_only_and_nominal_identity_does_not_imply_structural_compatibility() {
    rejects(
        "archetype A { init() {} highkey sidequest bad(A other) -> unit uses() { self = other; } } main_character {}",
        "LARP-T008",
    );
    rejects(
        "archetype A { init() {} } archetype B { init() {} } main_character { A value = new B(); }",
        "LARP-T001",
    );
    rejects("main_character { let value = self; }", "LARP-T001");
    rejects("main_character { let value = new Unknown(); }", "LARP-T005");
    rejects(
        "archetype A { init(mf x) {} } main_character { let value = new A(); }",
        "LARP-T006",
    );
}

#[test]
fn private_access_is_lexical_and_does_not_depend_on_receiver_aliases() {
    let class = "archetype A { mf field; init() { self.field = 1; } sidequest hidden() -> mf uses(read) { return self.field; } }";
    for statement in [
        "yap(value.field);",
        "value.field = 2;",
        "value.hidden();",
        "let hidden = value.hidden;",
        "let capture = () => value.field;",
    ] {
        rejects(
            &format!("{class} main_character {{ let value = new A(); {statement} }}"),
            "LARP-T010",
        );
    }
    rejects(
        &format!(
            "{class} archetype B {{ init(A value) {{ let private = value.field; }} }} main_character {{}}"
        ),
        "LARP-T010",
    );
    run(
        r#"
        archetype A {
            mf field;
            init(mf field) { self.field = field; }
            highkey sidequest other(A value) -> mf uses(read) { return value.field; }
        }
        main_character { let first = new A(1); let second = new A(2); yap(first.other(second)); }
    "#,
        "2\n",
    );
}

#[test]
fn object_fields_compose_with_fixed_array_places_and_compound_assignment_order() {
    run(
        r#"
        archetype State {
            highkey hediJeans<mf, 2> row;
            highkey mf value;
            init() { self.row = [1, 2]; self.value = 10; }
        }
        sidequest change(State state) -> mf uses(read, write, abort) { state.row[1] = 99; state.value = 100; return 7; }
        main_character {
            let state = new State();
            state.row[0] = change(state);
            yap(state.row[0]); yap(state.row[1]);
            state.value = 10;
            state.value += change(state);
            yap(state.value);
            hediJeans<State, 1> fixed = [state];
            let mutate = () => { fixed[0].value = 49; };
            mutate(); yap(state.value);
        }
    "#,
        "7\n99\n17\n49\n",
    );
}

#[test]
fn fields_and_methods_preserve_numeric_literal_context_and_effect_bounds() {
    run(
        r#"
        archetype Wide {
            highkey unc value;
            init() { self.value = fortnine; let copied = 1 + self.value; self.value = copied; }
            highkey sidequest get() -> unc uses(read) { return self.value; }
            highkey sidequest isSome() -> unc uses(read) { return self.value; }
        }
        main_character { let wide = new Wide(); yap(1 + wide.get()); yap(1 + wide.value); yap(1 + wide.isSome()); }
    "#,
        "51\n51\n51\n",
    );
    for body in ["return value.field;", "return value.get();"] {
        rejects(
            &format!(
                "archetype A {{ highkey mf field; init() {{ self.field = 0; }} highkey sidequest get() -> mf uses(read) {{ return self.field; }} }} sidequest pure(A value) -> mf uses() {{ {body} }} main_character {{}}"
            ),
            "LARP-E001",
        );
    }
    rejects(
        "archetype A { highkey mf field; init() { self.field = 0; } } sidequest f(A value) -> unit uses(read) { value.field = 1; } main_character {}",
        "LARP-E001",
    );
    rejects(
        "archetype A { init() {} } sidequest f() -> A uses() { return new A(); } main_character {}",
        "LARP-E001",
    );
}

#[test]
fn duplicate_declarations_missing_constructors_and_incomplete_fields_have_source_errors() {
    for (text, code) in [
        ("archetype A {} main_character {}", "LARP-T011"),
        (
            "archetype A { init() {} init() {} } main_character {}",
            "LARP-T011",
        ),
        ("archetype mf { init() {} } main_character {}", "LARP-N002"),
        (
            "archetype A { init() {} } archetype A { init() {} } main_character {}",
            "LARP-N002",
        ),
        (
            "archetype A { mf x; mf x; init() {} } main_character {}",
            "LARP-N002",
        ),
        (
            "archetype A { mf x; init() { self.x = 1; } sidequest x() -> unit {} } main_character {}",
            "LARP-N002",
        ),
        (
            "archetype A { unit field; init() {} } main_character {}",
            "LARP-T001",
        ),
        (
            "archetype A { Unknown field; init() {} } main_character {}",
            "LARP-T005",
        ),
    ] {
        rejects(text, code);
    }
    let source = source(
        "archetype A { mf field; init() { let read = self.field; self.field = 1; } } main_character {}",
    );
    let errors = compile(&source).err().unwrap();
    assert_eq!(errors[0].code.id, "LARP-T011");
    assert_eq!(source.slice(errors[0].primary.span), Some("self.field"));
}

#[test]
fn constructor_abort_has_no_observation_and_keeps_exact_source_span() {
    let source = source(
        "archetype A { mf field; init() { crashout(\"stop\"); } } main_character { let value = new A(); }",
    );
    let compiled = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let mut output = Vec::new();
    let diagnostic =
        larp_vm::execute(&compiled.bytecode, &mut output, Default::default()).unwrap_err();
    assert_eq!(diagnostic.code.id, "LARP-R003");
    assert_eq!(
        source.slice(diagnostic.primary.span),
        Some("crashout(\"stop\")")
    );
    assert!(output.is_empty());
}

#[test]
fn source_object_cycles_and_bound_method_roots_survive_and_are_collected() {
    let source = source(
        r#"
        archetype Node {
            highkey baddie<Node> next;
            mf value;
            init(mf value) { self.value = value; self.next = no_bitches; }
            highkey sidequest get() -> mf uses(read) { return self.value; }
        }
        sidequest cycle() -> unit uses(read, write, abort) {
            let first = new Node(1); let second = new Node(2);
            first.next = some(second); second.next = some(first);
        }
        sidequest bound() -> fn() -> mf uses(read) uses(read, write, abort) {
            let value = new Node(49); return value.get;
        }
        main_character {
            let live = bound();
            npc loop (mf i in 0..500) { cycle(); }
            yap(live());
        }
    "#,
    );
    let compiled = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let mut output = Vec::new();
    let stats = larp_vm::execute_with_stats(
        &compiled.bytecode,
        &mut output,
        larp_vm::Limits {
            gc_stress: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(output, b"49\n");
    assert_eq!(stats.allocations, stats.reclaimed);
    assert!(stats.peak_live_objects < 20, "{stats:?}");
}
