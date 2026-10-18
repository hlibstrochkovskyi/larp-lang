use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "roles.larp", text)
}

fn run(text: &str, expected: &str) {
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
        let stats = larp_vm::execute_with_stats(
            &compilation.bytecode,
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
        "Frontend errors must not reach the bytecode verifier"
    );
    assert!(
        errors
            .iter()
            .all(|error| source.slice(error.primary.span).is_some())
    );
}

const SCORED: &str = r#"
    role Scored { sidequest score() -> mf uses(read); }
    archetype A : Scored {
        highkey mf value;
        init(mf value) { self.value = value; }
        highkey sidequest score() -> mf uses(read) { return self.value; }
    }
    archetype B : Scored {
        mf value;
        init(mf value) { self.value = value; }
        highkey sidequest score() -> mf uses(read) { return self.value; }
    }
"#;

#[test]
fn role_dispatch_uses_the_concrete_class_and_preserves_shared_identity() {
    run(
        &format!(
            r#"{SCORED}
        sidequest read(Scored value) -> mf uses(read) {{ return value.score(); }}
        main_character {{
            let original = new A(2);
            Scored first = original;
            Scored same = original;
            Scored second = new B(49);
            no_cap(first == same); no_cap(first != second);
            yap(read(first)); yap(read(second));
            original.value = 7;
            yap(first.score());
        }}
    "#
        ),
        "2\n49\n7\n",
    );
}

#[test]
fn multiple_role_tables_follow_role_signature_order_and_preserve_parameter_coercions() {
    run(
        r#"
        role First {
            sidequest first() -> mf uses();
            sidequest accept(Second other) -> mf uses();
        }
        role Second {
            sidequest second() -> mf uses();
            sidequest last() -> mf uses();
        }
        archetype A : Second, First {
            init() {}
            highkey sidequest last() -> mf uses() { return 4; }
            highkey sidequest accept(Second other) -> mf uses() { return other.second(); }
            highkey sidequest first() -> mf uses() { return 1; }
            highkey sidequest second() -> mf uses() { return 2; }
            highkey sidequest asFirst() -> First uses() { return self; }
        }
        main_character {
            let value = new A(); First first = value; Second second = value;
            yap(first.first()); yap(first.accept(value)); yap(second.second()); yap(second.last());
            yap(value.asFirst().first());
        }
    "#,
        "1\n2\n2\n4\n1\n",
    );
}

#[test]
fn contextual_role_coercions_cover_bindings_arguments_returns_fields_and_literal_payloads() {
    run(
        &format!(
            r#"{SCORED}
        sidequest read(Scored value) -> mf uses(read) {{ return value.score(); }}
        sidequest asRole(A value) -> Scored uses() {{ return value; }}
        sidequest create() -> Scored uses(read,write,abort) {{ return new B(3); }}
        archetype Holder {{
            highkey Scored value;
            init(A value) {{ self.value = value; }}
        }}
        main_character {{
            let first = new A(1);
            Scored value = first;
            value = new B(2);
            yap(value.score()); yap(read(first)); yap(asRole(first).score()); yap(create().score());
            let holder = new Holder(first); yap(holder.value.score());
            mfs<Scored> values = [first, new B(4)];
            values.push(new A(5));
            values.forEach(x => yap(x.score()));
            hediJeans<Scored,1> fixed = [first]; yap(fixed[0].score());
            baddie<Scored> optional = some(new B(6)); yap(optional.copium(first).score());
            result<Scored,text> good = win(new B(7)); yap(good.cope(err => first).score());
            fn(A)->Scored uses() convert = x => x;
            yap(convert(first).score());
        }}
    "#
        ),
        "2\n1\n1\n3\n1\n1\n4\n5\n1\n6\n7\n1\n",
    );
}

#[test]
fn role_conversion_never_recursively_changes_existing_container_or_function_types() {
    for body in [
        "mfs<A> original = [new A(1)]; mfs<Scored> converted = original;",
        "hediJeans<A,1> original = [new A(1)]; hediJeans<Scored,1> converted = original;",
        "baddie<A> original = some(new A(1)); baddie<Scored> converted = original;",
        "result<A,text> original = win(new A(1)); result<Scored,text> converted = original;",
        "fn(A)->A uses() original = x => x; fn(A)->Scored uses() converted = original;",
        "Scored value = new A(1); A concrete = value;",
    ] {
        rejects(
            &format!("{SCORED} main_character {{ {body} }}"),
            "LARP-T001",
        );
    }
    rejects(
        "role R {} role S {} archetype A : R,S { init() {} } main_character { R r = new A(); S s = r; }",
        "LARP-T001",
    );
    rejects(
        "role R { sidequest score() -> mf uses(); } archetype A { init() {} highkey sidequest score() -> mf uses() { return 1; } } main_character { R r = new A(); }",
        "LARP-T001",
    );
}

#[test]
fn role_implementations_require_public_exact_signatures_and_effect_subsets() {
    for member in [
        "",
        "sidequest score(mf bonus) -> mf uses() { return bonus; }",
        "highkey sidequest score(unc bonus) -> mf uses() { return 1; }",
        "highkey sidequest score(mf bonus) -> unc uses() { return 1; }",
        "highkey sidequest score() -> mf uses() { return 1; }",
        "highkey mf score;",
        "highkey sidequest score(mf bonus) -> mf uses(observe) { yap(bonus); return bonus; }",
    ] {
        rejects(
            &format!(
                "role R {{ sidequest score(mf bonus) -> mf uses(read); }} archetype A : R {{ init() {{}} {member} }} main_character {{}}"
            ),
            "LARP-T013",
        );
    }
    rejects(
        "role R { sidequest f(fn()->mf uses() callback) -> mf uses(); } archetype A : R { init() {} highkey sidequest f(fn()->mf uses(read) callback) -> mf uses() { return 1; } } main_character {}",
        "LARP-T013",
    );
    run(
        "role R { sidequest f() -> mf uses(read,observe); } archetype A : R { init() {} highkey sidequest f() -> mf uses() { return 49; } } main_character { R r = new A(); yap(r.f()); }",
        "49\n",
    );
}

#[test]
fn nominal_declaration_errors_and_role_instantiation_have_frontend_diagnostics() {
    for (text, code) in [
        ("role R {} role R {} main_character {}", "LARP-N002"),
        (
            "role R {} archetype R { init() {} } main_character {}",
            "LARP-N002",
        ),
        ("role mf {} main_character {}", "LARP-N002"),
        (
            "role R { sidequest f() -> unit; sidequest f() -> unit; } main_character {}",
            "LARP-N002",
        ),
        (
            "role R {} archetype A : R,R { init() {} } main_character {}",
            "LARP-T013",
        ),
        (
            "archetype A : Missing { init() {} } main_character {}",
            "LARP-T013",
        ),
        (
            "archetype A { init() {} } archetype B : A { init() {} } main_character {}",
            "LARP-T013",
        ),
        (
            "role R {} main_character { let value = new R(); }",
            "LARP-T005",
        ),
    ] {
        rejects(text, code);
    }
}

#[test]
fn bound_role_methods_snapshot_the_receiver_but_observe_later_object_mutation() {
    run(
        &format!(
            r#"{SCORED}
        sidequest saved() -> fn()->mf uses(read) uses(read,write,abort) {{
            Scored receiver = new A(49);
            return receiver.score;
        }}
        main_character {{
            let original = new A(1);
            Scored current = original;
            let bound = current.score;
            current = new B(100);
            original.value = 7;
            let returned = saved();
            npc loop (mf i in 0..200) {{ let garbage = new A(i); let temporary = garbage.score; }}
            yap(bound()); yap(current.score()); yap(returned());
        }}
    "#
        ),
        "7\n100\n49\n",
    );
}

#[test]
fn concrete_extensions_use_exact_static_receiver_types_and_can_be_bound_values() {
    run(
        &format!(
            r#"{SCORED}
        sidequest mf.tag() -> text uses() {{ return "mf"; }}
        sidequest unc.tag() -> text uses() {{ return "unc"; }}
        sidequest tag() -> text uses() {{ return "global"; }}
        sidequest A.label() -> text uses() {{ return "class"; }}
        sidequest Scored.label() -> text uses() {{ return "role"; }}
        sidequest mfs<mf>.first() -> mf uses(read,abort) {{ return self[0]; }}
        sidequest hediJeans<unc,1>.first() -> unc uses(abort) {{ return self[0]; }}
        sidequest fn(mf)->mf uses().apply(mf value) -> mf uses() {{ return self(value); }}
        main_character {{
            mf small = 1; unc wide = 1;
            yap(small.tag()); yap(wide.tag()); yap(tag());
            let captured = small.tag;
            small = 2;
            yap(captured());
            let concrete = new A(1); Scored roleValue = concrete;
            yap(concrete.label()); yap(roleValue.label());
            mfs<mf> values = [49]; yap(values.first());
            hediJeans<unc,1> fixed = [20]; yap(fixed.first());
            fn(mf)->mf uses() callback = x => x; yap(callback.apply(7));
        }}
    "#
        ),
        "mf\nunc\nglobal\nmf\nclass\nrole\n49\n20\n7\n",
    );
    rejects(
        &format!(
            "{SCORED} sidequest Scored.onlyRole() -> mf uses() {{ return 1; }} main_character {{ let value = new A(1); yap(value.onlyRole()); }}"
        ),
        "LARP-T001",
    );
    rejects(
        "sidequest mfs<mf>.first() -> mf uses(read,abort) { return self[0]; } main_character { mfs<unc> values = [1]; yap(values.first()); }",
        "LARP-T001",
    );
    rejects(
        "sidequest fn()->mf uses().answer() -> mf uses() { return self(); } main_character { fn()->mf uses(read) callback = () => 1; yap(callback.answer()); }",
        "LARP-T001",
    );
}

#[test]
fn role_and_extension_calls_evaluate_receivers_before_arguments_and_save_bound_values() {
    run(
        r#"
        role Add { sidequest add(mf amount) -> mf uses(read,abort); }
        archetype A : Add {
            mf value;
            init() { self.value = 10; }
            highkey sidequest add(mf amount) -> mf uses(read,abort) { return self.value + amount; }
        }
        sidequest receiver() -> Add uses(read,write,abort,observe) { yap("receiver"); return new A(); }
        sidequest number() -> mf uses(observe) { yap("number"); return 20; }
        sidequest argument() -> mf uses(observe) { yap("argument"); return 2; }
        sidequest mf.plus(mf amount) -> mf uses(abort) { return self + amount; }
        sidequest mf.memo() -> fn()->mf uses() uses() { return () => self; }
        main_character {
            yap(receiver().add(argument()));
            yap(number().plus(argument()));
            mf original = 1;
            let bound = original.plus;
            let memo = original.memo();
            original = 99;
            yap(bound(2)); yap(memo());
            let factory = (mf value) => value.plus;
            let returned = factory(40); yap(returned(9));
        }
    "#,
        "receiver\nargument\n12\nnumber\nargument\n22\n3\n1\n49\n",
    );
}

#[test]
fn instance_and_builtin_members_win_over_same_named_extensions() {
    run(
        r#"
        archetype A {
            highkey fn()->mf uses() callback;
            init() { self.callback = () => 3; }
            highkey sidequest score() -> mf uses() { return 1; }
        }
        sidequest A.score() -> mf uses() { return 2; }
        sidequest A.callback() -> mf uses() { return 4; }
        sidequest mfs<mf>.size() -> mf uses() { return 99; }
        sidequest text.size() -> mf uses() { return 99; }
        main_character {
            let value = new A();
            yap(value.score()); let bound = value.score; yap(bound());
            yap(value.callback()); let callback = value.callback; yap(callback());
            mfs<mf> values = [1,2]; yap(values.size()); yap("🦀a".size());
        }
    "#,
        "1\n1\n3\n3\n2\n2\n",
    );
    rejects(
        "archetype A { init() {} highkey sidequest f(mf x) -> mf uses() { return x; } } sidequest A.f() -> mf uses() { return 1; } main_character { let value = new A(); yap(value.f()); }",
        "LARP-T006",
    );
    rejects(
        "archetype A { highkey mf f; init() { self.f = 1; } } sidequest A.f() -> mf uses() { return 2; } main_character { let value = new A(); yap(value.f()); }",
        "LARP-T001",
    );
    rejects(
        "sidequest mfs<mf>.size(mf ignored) -> mf uses() { return ignored; } main_character { mfs<mf> values = []; yap(values.size(1)); }",
        "LARP-T006",
    );
    rejects(
        "role R { sidequest f(mf x) -> mf uses(); } sidequest R.f() -> mf uses() { return 1; } archetype A : R { init() {} highkey sidequest f(mf x) -> mf uses() { return x; } } main_character { R value = new A(); yap(value.f()); }",
        "LARP-T006",
    );
}

#[test]
fn inaccessible_members_do_not_fall_back_to_extensions_and_extensions_get_no_private_access() {
    for member in [
        "mf hidden;",
        "sidequest hidden() -> mf uses() { return 1; }",
    ] {
        let init = if member.starts_with("mf") {
            "self.hidden = 1;"
        } else {
            ""
        };
        rejects(
            &format!(
                "archetype A {{ {member} init() {{ {init} }} }} sidequest A.hidden() -> mf uses() {{ return 2; }} main_character {{ let value = new A(); value.hidden(); }}"
            ),
            "LARP-T010",
        );
    }
    for body in [
        "return self.hidden;",
        "return (() => self.hidden)();",
        "return self.secret();",
        "let bound = self.secret; return bound();",
    ] {
        rejects(
            &format!(
                "archetype A {{ mf hidden; init() {{ self.hidden = 1; }} sidequest secret() -> mf uses(read) {{ return self.hidden; }} }} sidequest A.expose() -> mf uses(read) {{ {body} }} main_character {{}}"
            ),
            "LARP-T010",
        );
    }
}

#[test]
fn extension_self_is_immutable_but_reference_receivers_can_mutate_public_state() {
    run(
        r#"
        archetype A { highkey mf value; init() { self.value = 0; } }
        sidequest A.set(mf value) -> unit uses(write) { self.value = value; }
        sidequest mfs<mf>.append(mf value) -> unit uses(write,abort) { self.push(value); }
        main_character { let value = new A(); value.set(49); yap(value.value); mfs<mf> values = []; values.append(20); yap(values[0]); }
    "#,
        "49\n20\n",
    );
    for text in [
        "sidequest mf.bad(mf other) -> unit uses() { self = other; } main_character {}",
        "sidequest hediJeans<mf,1>.bad() -> unit uses(abort) { self[0] = 2; } main_character {}",
    ] {
        rejects(text, "LARP-T008");
    }
    rejects(
        "sidequest unit.bad() -> unit uses() {} main_character {}",
        "LARP-T001",
    );
    rejects(
        "sidequest mf.same() -> unit uses() {} sidequest mf.same() -> unit uses() {} main_character {}",
        "LARP-N002",
    );
}

#[test]
fn role_and_extension_effect_bounds_survive_dispatch_bound_values_and_callbacks() {
    let prefix = "role R { sidequest read() -> mf uses(read,observe); } archetype A : R { init() {} highkey sidequest read() -> mf uses() { return 1; } }";
    run(
        &format!(
            "{prefix} sidequest direct(A value) -> mf uses() {{ return value.read(); }} main_character {{ let value = new A(); yap(direct(value)); }}"
        ),
        "1\n",
    );
    for function in [
        "sidequest bad(R value) -> mf uses(read) { return value.read(); }",
        "sidequest bad(R value) -> mf uses(read) { let bound = value.read; return bound(); }",
        "sidequest bad(R value) -> fn()->mf uses(read) uses() { return value.read; }",
    ] {
        let code = if function.contains("fn()->mf") {
            "LARP-T001"
        } else {
            "LARP-E001"
        };
        rejects(&format!("{prefix} {function} main_character {{}}"), code);
    }
    let extension = "sidequest mf.noisy() -> mf uses(observe) { yap(self); return self; }";
    rejects(
        &format!(
            "{extension} sidequest bad(mf value) -> mf uses() {{ return value.noisy(); }} main_character {{}}"
        ),
        "LARP-E001",
    );
    rejects(
        &format!(
            "{extension} sidequest bad(mf value) -> mf uses() {{ let bound = value.noisy; return bound(); }} main_character {{}}"
        ),
        "LARP-E001",
    );
    rejects(
        &format!(
            "{extension} main_character {{ mf value = 1; fn()->mf uses() erased = value.noisy; }}"
        ),
        "LARP-T001",
    );
    rejects(
        &format!(
            "{prefix} sidequest bad(mfs<R> values) -> mfs<mf> uses(read) {{ return values.map(value => value.read()); }} main_character {{}}"
        ),
        "LARP-E001",
    );
    rejects(
        "role Writer { sidequest write() -> unit uses(write); } archetype Bad { init(Writer external) { external.write(); } } main_character {}",
        "LARP-T012",
    );
}

#[test]
fn role_references_are_traced_through_cycles_and_returned_bound_methods() {
    let text = r#"
        role Reader { sidequest get() -> mf uses(read); }
        archetype Node : Reader {
            highkey baddie<Reader> next;
            mf value;
            init(mf value) { self.value = value; self.next = no_bitches; }
            highkey sidequest get() -> mf uses(read) { return self.value; }
        }
        sidequest cycle() -> unit uses(read,write,abort) {
            let a = new Node(1); let b = new Node(2);
            a.next = some(b); b.next = some(a);
        }
        sidequest saved() -> fn()->mf uses(read) uses(read,write,abort) {
            Reader value = new Node(49); return value.get;
        }
        main_character { let live = saved(); npc loop (mf i in 0..500) { cycle(); } yap(live()); }
    "#;
    let compilation = compile(&source(text)).unwrap_or_else(|errors| panic!("{errors:#?}"));
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
    assert_eq!(stats.allocations, stats.reclaimed);
    assert!(stats.peak_live_objects < 20, "{stats:?}");
}

#[test]
fn executable_roles_example_runs_through_the_compiler_and_vm() {
    run(
        include_str!("../../../examples/roles.larp"),
        "7\n49\n14\n98\n",
    );
}
