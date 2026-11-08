use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn checked(text: &str) -> (SourceFile, larp_compiler::Compilation) {
    let source = SourceFile::new(SourceId(0), "patterns.larp", text);
    let compilation = compile(&source).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(|error| larp_diagnostics::render(error, &source))
                .collect::<String>()
        )
    });
    (source, compilation)
}

fn run(text: &str, expected: &str) {
    let (source, compilation) = checked(text);
    let bytes = larp_artifact::encode(&compilation.bytecode, &[source]).unwrap();
    let loaded = larp_artifact::decode(&bytes).unwrap();
    for module in [&compilation.bytecode, &loaded.module] {
        for gc_stress in [false, true] {
            let mut output = Vec::new();
            let stats = larp_vm::execute_with_stats(
                module,
                &mut output,
                larp_vm::Limits {
                    gc_stress,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|error| panic!("{error:?}"));
            assert_eq!(output, expected.as_bytes(), "gc_stress={gc_stress}");
            assert_eq!(stats.allocations, stats.reclaimed);
        }
    }
}

fn rejects(text: &str, code: &str) {
    let source = SourceFile::new(SourceId(0), "bad-patterns.larp", text);
    let errors = compile(&source).err().expect("must fail source checking");
    assert!(
        errors.iter().any(|error| error.code.id == code),
        "{errors:#?}"
    );
    assert!(
        errors.iter().all(|error| error.code.id != "LARP-B001"),
        "source rejection must precede verification: {errors:#?}"
    );
    for error in errors {
        assert!(source.slice(error.primary.span).is_some());
    }
}

#[test]
fn nominal_variants_forward_layouts_extensions_and_first_class_constructors_execute() {
    run(
        r#"
        enum Outer { Hold(Inner), Empty }
        enum Inner { Score(mf), Missing }
        sidequest Outer.score() -> mf uses() {
            return match (self) {
                Outer::Hold(inner) => match (inner) { Inner::Score(score) => score, Inner::Missing => 0 },
                Outer::Empty => -1,
            };
        }
        main_character {
            fn(mf) -> Inner uses() wrap = Inner::Score;
            let outer = Outer::Hold(wrap(49));
            yap(outer.score());
            yap(Outer::Empty.score());
            mfs<mf> scores = [2, 4];
            mfs<Inner> values = scores.map(Inner::Score);
            values.forEach(item => yap(match (item) { Inner::Score(x) => x, _ => 0 }));
        }
    "#,
        "49\n-1\n2\n4\n",
    );
}

#[test]
fn every_builtin_sum_case_wildcard_and_payload_scope_execute() {
    run(
        r#"
        main_character {
            baddie<mf> present = some(49);
            baddie<mf> absent = no_bitches;
            result<mf,text> good = win(20);
            result<mf,text> bad = fumble("expected");
            mf x = 7;
            yap(match (present) { some(x) => x, no_bitches => 0 });
            yap(match (absent) { no_bitches => 4, some(_) => 0 });
            yap(match (good) { fumble(_) => 0, win(score) => score });
            yap(match (bad) { win(_) => "wrong", fumble(message) => message });
            yap(match (fr) { cap => "wrong", fr => "fr" });
            yap(match (cap) { fr => "wrong", _ => "cap" });
            yap(x);
        }
    "#,
        "49\n4\n20\nexpected\nfr\ncap\n7\n",
    );
}

#[test]
fn managed_enum_payloads_and_captured_binders_remain_gc_roots() {
    run(
        r#"
        enum Link { None, Next(Node) }
        archetype Node {
            highkey Link next;
            highkey mf value;
            init(mf value) { self.next = Link::None; self.value = value; }
        }
        main_character {
            Node node = new Node(49);
            node.next = Link::Next(node);
            mfs<Link> links = [node.next];
            fn() -> mf uses(read) read = match (links[0]) {
                Link::None => () => 0,
                Link::Next(item) => () => item.value,
            };
            mfs<Link> more = [Link::None, Link::Next(new Node(20))];
            yap(read());
            yap(match (more[1]) { Link::None => 0, Link::Next(value) => value.value });
        }
    "#,
        "49\n20\n",
    );
}

#[test]
fn structural_enum_equality_and_fixed_array_value_payloads_execute() {
    run(
        r#"
        enum Value { Empty, Pair(text, hediJeans<mf,2>) }
        main_character {
            hediJeans<mf,2> data = [2,4];
            Value copy = Value::Pair("aura", data);
            data[0] = 49;
            yap(copy == Value::Pair("aura", [2,4]));
            yap(copy != Value::Empty);
            yap(match (copy) { Value::Empty => 0, Value::Pair(_, saved) => saved[0] });
        }
    "#,
        "fr\nfr\n2\n",
    );
    rejects(
        "enum Ref { Empty, Values(mfs<mf>) } main_character { yap(Ref::Empty == Ref::Empty); }",
        "LARP-T001",
    );
    rejects("enum E { A } main_character { yap(E::A); }", "LARP-T001");
}

#[test]
fn match_blocks_preserve_return_inference_constructor_initialization_and_breaks() {
    run(
        r#"
        archetype Score {
            highkey mf value;
            init(verdict chosen) {
                match (chosen) { fr => { self.value = 49; }, cap => { self.value = 20; } };
            }
        }
        main_character {
            let choose = (verdict chosen) => {
                match (chosen) { fr => { return fortnine; }, cap => { return 20; } };
            };
            yap(choose(fr));
            yap(choose(cap));
            yap(new Score(cap).value);
            mf score = 0;
            goon {
                match (fr) { fr => { score = 49; touch_grass; }, cap => { score = -1; } };
            }
            yap(score);
        }
    "#,
        "49\n20\n20\n49\n",
    );
    rejects(
        "archetype A { mf value; init(verdict b) { match (b) { fr => { self.value = 1; }, cap => {} }; } } main_character {}",
        "LARP-T011",
    );
}

#[test]
fn match_context_nonreturning_arms_and_single_scrutinee_evaluation_execute() {
    run(
        r#"
        sidequest input() -> result<mf,text> uses(observe) { yap("once"); return win(49); }
        sidequest read() -> mf uses(observe) {
            return match (input()) { win(value) => value, fumble(error) => { yap(error); return -1; } };
        }
        main_character {
            unc wide = match (cap) { fr => 1, cap => fortnine };
            yap(wide);
            yap(read());
        }
    "#,
        "49\nonce\n49\n",
    );
    rejects(
        "main_character { let x = match (cap) { fr => 1, cap => fortnine }; }",
        "LARP-T041",
    );
    rejects(
        "sidequest pure() -> mf uses() { return match (fr) { fr => 1, cap => { yap(2); return 2; } }; } main_character {}",
        "LARP-E001",
    );
}

#[test]
fn malformed_coverage_patterns_and_nominal_constructors_are_source_errors() {
    for source in [
        "main_character { match (fr) { fr => 1 }; }",
        "main_character { match (fr) { fr => 1, fr => 2, cap => 3 }; }",
        "main_character { match (fr) { _ => 1, cap => 2 }; }",
        "main_character { match (49) { _ => 1 }; }",
        "enum E { A(mf,mf) } main_character { match (E::A(1,2)) { E::A(x,x) => x }; }",
        "enum A { Same } enum B { Same } main_character { match (A::Same) { B::Same => 0 }; }",
        "main_character { baddie<mf> x = some(1); match(x) { some => 1, no_bitches => 0 }; }",
        "enum E { A } main_character { match (E::A) { E::A() => 0 }; }",
    ] {
        rejects(source, "LARP-T016");
    }
    for source in [
        "enum E { A } main_character { E::A(); }",
        "enum E { A(mf) } main_character { E::A(1,2); }",
        "enum E { A, A } main_character {}",
        "enum E { A(E) } main_character {}",
        "enum A { X(baddie<B>) } enum B { X(fn() -> A) } main_character {}",
        "enum E { A(unit) } main_character {}",
    ] {
        rejects(source, "LARP-T015");
    }
}

#[test]
fn result_propagation_uses_outer_error_wrapper_and_restores_suppression() {
    run(
        r#"
        sidequest lift(text input) -> result<unc,text> uses(observe,abort) {
            sybau {
                yap("hidden");
                mf score = parseScore(input)?;
                return win(fraud<unc>(score));
            }
        }
        main_character {
            yap(lift("49").cope(error => 0));
            yap(lift("bad").cope(error => 20));
            sybau { result<unc,text> ignored = lift("bad"); yap("caller hidden"); }
            yap("visible");
        }
    "#,
        "49\n20\nvisible\n",
    );
}

#[test]
fn propagation_preserves_left_to_right_arguments_and_indexed_place_order() {
    run(
        r#"
        sidequest first() -> mf uses(observe) { yap("first"); return 1; }
        sidequest last() -> mf uses(observe) { yap("last"); return 3; }
        sidequest failed() -> result<mf,text> uses(observe) { yap("failed"); return fumble("expected"); }
        sidequest sum() -> result<mf,text> uses(observe,abort) {
            mf value = first() + failed()? + last();
            return win(value);
        }
        sidequest index() -> mf uses(observe) { yap("index"); return 0; }
        sidequest assign(mfs<mf> values) -> result<mf,text> uses(read,write,observe,abort) {
            values[index()] = failed()?;
            return win(1);
        }
        main_character {
            yap(sum().cope(error => 0));
            mfs<mf> values = [49];
            yap(assign(values).cope(error => 0));
            yap(values[0]);
        }
    "#,
        "first\nfailed\n0\nindex\nfailed\n0\n49\n",
    );
}

#[test]
fn propagation_supports_contextual_lambdas_chains_hints_and_memoized_lazy_errors() {
    run(
        r#"
        sidequest wide() -> result<unc,text> uses() { return win(fortnine); }
        sidequest addWide() -> result<unc,text> { return win(3_000_000_000 + wide()?); }
        sidequest nested() -> result<result<mf,text>,text> uses() { return win(win(49)); }
        sidequest readNested() -> result<mf,text> uses() { return win(nested()??); }
        sidequest lazyError() -> mf uses(abort) {
            nonchalant result<mf,text> cached = win(parseScore("bad")?);
            mf first = cached.cope(error => 20);
            return first + cached.cope(error => 29);
        }
        main_character {
            text input = "49";
            fn() -> result<mf,text> uses() parse = () => win(parseScore(input)?);
            yap(parse().cope(error => 0));
            yap(addWide().cope(error => 0));
            yap(readNested().cope(error => 0));
            yap(lazyError());
            result<verdict,text> stop = win(cap);
            fn() -> result<verdict,text> uses() skip = () => win(cap && stop?);
            yap(skip().cope(error => fr));
        }
    "#,
        "49\n3000000049\n49\n49\ncap\n",
    );
}

#[test]
fn propagation_requires_exact_errors_and_known_lexical_result_boundaries() {
    rejects(
        "main_character { mf value = parseScore(\"49\")?; }",
        "LARP-T017",
    );
    rejects(
        "sidequest f() -> result<mf,unc> { mf x = parseScore(\"49\")?; return win(x); } main_character {}",
        "LARP-T017",
    );
    rejects(
        "sidequest f() -> result<mf,text> { baddie<mf> x = some(1); return win(x?); } main_character {}",
        "LARP-T017",
    );
    rejects(
        "sidequest f() -> result<mf,text> { return parseScore(\"49\")?; } main_character {}",
        "LARP-T001",
    );
    rejects(
        "main_character { let f = () => { result<mf,text> x = win(1); mf value = x?; return x; }; }",
        "LARP-T017",
    );
    rejects(
        "main_character { let f = (verdict b) => { result<mf,text> x = win(1); if(b) {return x;} mf value = x?; return x; }; }",
        "LARP-T017",
    );
    rejects(
        "archetype A { mf x; init() { self.x = parseScore(\"49\")?; } } main_character {}",
        "LARP-T012",
    );
    rejects(
        "main_character { nonchalant mf x = parseScore(\"49\")?; }",
        "LARP-T017",
    );
}

#[test]
fn snapshot_enums_and_region_local_error_returns_preserve_scenario_rules() {
    run(
        r#"
        enum Mood { Calm, Aura(mf) }
        archetype Founder {
            highkey mf aura;
            highkey Mood mood;
            init() { self.aura = 0; self.mood = Mood::Calm; }
        }
        sidequest attempt(Founder bro) -> result<mf,text> uses(read,write,abort) {
            bro.aura += 1;
            mf score = parseScore("bad")?;
            return win(score);
        }
        main_character {
            Founder bro = new Founder();
            larp (bro) as Trial {
                result<mf,text> attempted = attempt(bro);
                fn() -> result<mf,text> uses() local = () => win(parseScore("bad")?);
                bro.mood = Mood::Aura(local().cope(error => 49));
                canon bro.aura;
                canon bro.mood;
            }
            yap(bro.aura);
            yap(match(bro.mood) { Mood::Calm => 0, Mood::Aura(value) => value });
        }
    "#,
        "1\n49\n",
    );
    rejects(
        "archetype A { highkey mf x; init(){self.x=0;} } main_character { A a = new A(); larp(a) as Trial { mf x = parseScore(\"49\")?; } }",
        "LARP-S005",
    );
    rejects(
        "enum E { Value(mf), Reference(mfs<mf>) } archetype A { highkey E x; init(){self.x=E::Value(0);} } main_character { A a=new A(); larp(a) as Trial {} }",
        "LARP-S001",
    );
    rejects(
        "enum E { Value(mf), Reference(mfs<mf>) } archetype A { highkey mf x; init(){self.x=0;} } main_character { A a=new A(); larp(a) as Trial { E value = E::Value(49); } }",
        "LARP-E002",
    );
}

#[test]
fn expression_lambda_match_returns_do_not_overwrite_inferred_function_results() {
    run(
        r#"
        main_character {
            let chosen = (verdict b) => match (b) { fr => { return fortnine; }, cap => { return 20; } };
            yap(chosen(fr));
            yap(chosen(cap));
            fn(verdict) -> unc explicit = b => match (b) { fr => { return fortnine; }, cap => 20 };
            yap(explicit(cap));
            let mixed = (verdict b) => match (b) { fr => { return 49; }, cap => 20 };
            yap(mixed(cap));
        }
    "#,
        "49\n20\n20\n20\n",
    );
    rejects(
        "main_character { let f = (verdict b) => match (b) { fr => { return fortnine; }, cap => 20 }; }",
        "LARP-T001",
    );
}

fn module_project(entry: &str, files: &[(&str, &str)]) -> larp_compiler::project::Project {
    use larp_compiler::project::{self, ModuleSource};
    project::load(
        ModuleSource {
            identity: "entry".into(),
            name: "entry.larp".into(),
            text: entry.into(),
        },
        &mut |_importer: &str, request: &str| {
            files
                .iter()
                .find(|(path, _)| *path == request)
                .map(|(_, text)| ModuleSource {
                    identity: request.to_owned(),
                    name: request.to_owned(),
                    text: (*text).to_owned(),
                })
                .ok_or_else(|| format!("missing {request}"))
        },
    )
    .unwrap_or_else(|failure| panic!("{failure:?}"))
}

#[test]
fn exported_enum_identity_and_qualified_constructor_functions_work_across_modules() {
    let project = module_project(
        r#"
        import "types.larp" as left;
        import "types.larp" as right;
        main_character {
            fn(mf) -> left::E uses() wrap = right::E::Score;
            left::E value = wrap(49);
            yap(match(value) { left::E::Score(score) => score, right::E::Empty => 0 });
        }
    "#,
        &[("types.larp", "highkey enum E { Empty, Score(mf) }")],
    );
    let compilation = larp_compiler::compile_project(&project).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(|error| larp_diagnostics::render_sources(error, project.sources()))
                .collect::<String>()
        )
    });
    let bytes = larp_artifact::encode(&compilation.bytecode, project.sources()).unwrap();
    let loaded = larp_artifact::decode(&bytes).unwrap();
    for module in [&compilation.bytecode, &loaded.module] {
        for gc_stress in [false, true] {
            let mut output = Vec::new();
            larp_vm::execute(
                module,
                &mut output,
                larp_vm::Limits {
                    gc_stress,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(output, b"49\n");
        }
    }
}

#[test]
fn module_enum_privacy_nominality_and_alias_ambiguity_are_checked_before_bytecode() {
    for (entry, files, code) in [
        (
            "import \"types.larp\" as lib; main_character {}",
            vec![(
                "types.larp",
                "enum Hidden { A } highkey enum Public { Value(result<Hidden,text>) }",
            )],
            "LARP-M006",
        ),
        (
            "import \"types.larp\" as lib; main_character {}",
            vec![(
                "types.larp",
                "archetype Hidden { init() {} } highkey enum Public { Value(fn() -> Hidden) }",
            )],
            "LARP-M006",
        ),
        (
            "import \"types.larp\" as lib; main_character {}",
            vec![(
                "types.larp",
                "enum Hidden { A } highkey sidequest leak(Hidden value) -> mf {return 0;}",
            )],
            "LARP-M006",
        ),
        (
            "import \"a.larp\" as a; import \"b.larp\" as b; main_character { a::E value = b::E::Same; }",
            vec![
                ("a.larp", "highkey enum E { Same }"),
                ("b.larp", "highkey enum E { Same }"),
            ],
            "LARP-T001",
        ),
        (
            "import \"types.larp\" as lib; enum lib { A } main_character {}",
            vec![("types.larp", "highkey enum E { A }")],
            "LARP-T015",
        ),
    ] {
        let project = module_project(entry, &files);
        let errors = larp_compiler::compile_project(&project)
            .err()
            .expect("must reject module enum interface");
        assert!(
            errors.iter().any(|error| error.code.id == code),
            "{errors:?}"
        );
        assert!(errors.iter().all(|error| error.code.id != "LARP-B001"));
    }
}

#[test]
fn built_in_pattern_meanings_and_nested_binder_captures_ignore_value_shadowing() {
    run(
        r#"
        sidequest win(mf input) -> mf uses() { return input; }
        main_character {
            mf value = 49;
            fn() -> mf uses() read = () => match (parseScore("bad")) { win(value) => value, fumble(error) => value };
            yap(read());
            yap(match(parseScore("20")) { win(x) => x, fumble(error) => 0 });
        }
    "#,
        "49\n20\n",
    );
    rejects(
        "enum E { A(mf) } main_character { match(E::A(49)) { E::A(hidden) => hidden }; yap(hidden); }",
        "LARP-N001",
    );
}
