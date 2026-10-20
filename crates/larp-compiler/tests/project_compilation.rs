use std::collections::HashMap;

use larp_compiler::{Compilation, compile_project, project};
use larp_diagnostics::Diagnostic;
use larp_source::SourceId;

fn project(entry: &str, files: &[(&str, &str)]) -> project::Project {
    let files: HashMap<_, _> = files.iter().copied().collect();
    project::load(
        project::ModuleSource {
            identity: "entry".into(),
            name: "entry.larp".into(),
            text: entry.into(),
        },
        &mut |_importer: &str, request: &str| {
            files
                .get(request)
                .map(|text| project::ModuleSource {
                    identity: request.into(),
                    name: format!("{request}.larp"),
                    text: text.to_string(),
                })
                .ok_or_else(|| format!("Cannot resolve {request}"))
        },
    )
    .unwrap_or_else(|failure| panic!("Graph failed: {:?}", failure.diagnostics))
}

fn diagnostics(project: &project::Project, errors: &[Diagnostic]) -> String {
    errors
        .iter()
        .map(|error| {
            larp_diagnostics::render(
                error,
                &project.sources()[error.primary.span.source().0 as usize],
            )
        })
        .collect()
}

fn compile(project: &project::Project) -> Compilation {
    compile_project(project).unwrap_or_else(|errors| panic!("{}", diagnostics(project, &errors)))
}

fn run(entry: &str, files: &[(&str, &str)], expected: &str) {
    let project = project(entry, files);
    let compilation = compile(&project);
    let bytes = larp_artifact::encode(&compilation.bytecode, project.sources()).unwrap();
    let loaded = larp_artifact::decode(&bytes).unwrap();
    assert_eq!(loaded.sources.len(), project.sources().len());
    for (original, embedded) in project.sources().iter().zip(&loaded.sources) {
        assert_eq!(original.name(), embedded.name());
        assert_eq!(original.text(), embedded.text());
    }
    for (mode, module) in [
        ("source", &compilation.bytecode),
        ("artifact", &loaded.module),
    ] {
        for gc_stress in [false, true] {
            let mut output = Vec::new();
            let mut inspection = Vec::new();
            let stats = larp_vm::execute_with_observers(
                module,
                &mut output,
                &mut inspection,
                larp_vm::Limits {
                    gc_stress,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|error| panic!("{}", diagnostics(&project, &[*error])));
            assert_eq!(
                output,
                expected.as_bytes(),
                "mode={mode}, gc_stress={gc_stress}"
            );
            assert!(inspection.is_empty());
            assert_eq!(stats.allocations, stats.reclaimed);
        }
    }
}

fn rejects(
    entry: &str,
    files: &[(&str, &str)],
    accepted_codes: &[&str],
    expected_source: SourceId,
) {
    let project = project(entry, files);
    let errors = compile_project(&project)
        .err()
        .expect("Expected source rejection");
    assert!(
        errors
            .iter()
            .any(|error| accepted_codes.contains(&error.code.id)
                && error.primary.span.source() == expected_source),
        "{}",
        diagnostics(&project, &errors)
    );
    assert!(
        errors.iter().all(|error| error.code.id != "LARP-B001"),
        "Frontend rejection must precede bytecode verification: {}",
        diagnostics(&project, &errors)
    );
    for error in &errors {
        for label in std::iter::once(&error.primary).chain(&error.related) {
            assert!(
                project.sources()[label.span.source().0 as usize]
                    .slice(label.span)
                    .is_some()
            );
        }
    }
}

#[test]
fn imported_functions_bind_private_helpers_in_their_defining_module() {
    run(
        r#"
        import "math" as math;
        sidequest helper(mf value) -> mf uses(abort) { return value + 100; }
        main_character { yap(math::twice(20)); yap(helper(1)); }
    "#,
        &[(
            "math",
            r#"
        highkey sidequest twice(mf value) -> mf uses(abort) { return helper(value) + helper(value); }
        lowkey sidequest helper(mf value) -> mf uses(abort) { return value + 1; }
    "#,
        )],
        "42\n101\n",
    );
}

#[test]
fn module_qualifiers_coexist_with_local_values_and_identical_aliases_in_other_files() {
    run(
        r#"
        import "lib" as lib;
        main_character { mf lib = 7; yap(lib::answer() + lib); }
    "#,
        &[
            (
                "lib",
                r#"
        import "leaf" as lib;
        highkey sidequest answer() -> mf uses() { return lib::answer(); }
    "#,
            ),
            (
                "leaf",
                "highkey sidequest answer() -> mf uses() { return 42; }",
            ),
        ],
        "49\n",
    );
}

#[test]
fn diamond_imports_share_the_same_nominal_class_identity() {
    run(
        r#"
        import "left" as left; import "right" as right; import "common" as common;
        main_character {
            common::Box box = left::make(49);
            yap(right::read(box));
        }
    "#,
        &[
            (
                "left",
                r#"
        import "common" as shared;
        highkey sidequest make(mf value) -> shared::Box uses(read,write,abort) { return new shared::Box(value); }
    "#,
            ),
            (
                "right",
                r#"
        import "common" as common;
        highkey sidequest read(common::Box box) -> mf uses(read) { return box.value; }
    "#,
            ),
            (
                "common",
                r#"
        highkey archetype Box { highkey mf value; init(mf value) { self.value = value; } }
    "#,
            ),
        ],
        "49\n",
    );
}

#[test]
fn equal_class_spellings_in_different_modules_keep_distinct_runtime_behavior() {
    run(
        r#"
        import "first" as first; import "second" as second;
        main_character {
            first::Box a = new first::Box(); second::Box b = new second::Box();
            yap(a.value()); yap(b.value());
        }
    "#,
        &[
            (
                "first",
                r#"
        highkey archetype Box { init() {} highkey sidequest value() -> mf uses() { return 1; } }
    "#,
            ),
            (
                "second",
                r#"
        highkey archetype Box { init() {} highkey sidequest value() -> mf uses() { return 2; } }
    "#,
            ),
        ],
        "1\n2\n",
    );
}

#[test]
fn equal_class_spellings_in_different_modules_are_not_assignable() {
    rejects(
        r#"
        import "first" as first; import "second" as second;
        main_character { first::Box a = new second::Box(); }
    "#,
        &[
            ("first", "highkey archetype Box { init() {} }"),
            ("second", "highkey archetype Box { init() {} }"),
        ],
        &["LARP-T001"],
        SourceId(0),
    );
}

#[test]
fn imported_constructors_methods_and_nested_private_access_execute_normally() {
    run(
        r#"
        import "model" as model;
        main_character { let bro = new model::Founder(49); yap(bro.score()); yap(bro.reader()()); }
    "#,
        &[(
            "model",
            r#"
        highkey archetype Founder {
            lowkey mf aura;
            init(mf aura) { self.aura = aura; }
            lowkey sidequest secret() -> mf uses(read) { return self.aura; }
            highkey sidequest score() -> mf uses(read) { return self.secret(); }
            highkey sidequest reader() -> fn()->mf uses(read) uses() {
                return () => self.aura;
            }
        }
    "#,
        )],
        "49\n49\n",
    );
}

#[test]
fn imports_do_not_grant_private_field_or_method_access() {
    let model = r#"
        highkey archetype Founder {
            lowkey mf aura;
            init() { self.aura = 49; }
            lowkey sidequest secret() -> mf uses(read) { return self.aura; }
        }
    "#;
    for operation in ["yap(bro.aura);", "yap(bro.secret());", "bro.aura = 0;"] {
        rejects(
            &format!(
                r#"import "model" as model;
            main_character {{ let bro = new model::Founder(); {operation} }}"#
            ),
            &[("model", model)],
            &["LARP-T010"],
            SourceId(0),
        );
    }
}

#[test]
fn imported_roles_allow_nominal_implementation_dispatch_and_bound_methods() {
    run(
        r#"
        import "api" as api; import "model" as model;
        main_character {
            api::Scored bro = new model::Founder(49);
            yap(bro.score()); let callback = bro.score; yap(callback());
            mfs<api::Scored> bros = [new model::Founder(7)];
            bros.forEach(x => yap(x.score()));
        }
    "#,
        &[
            (
                "api",
                r#"
        highkey role Scored { sidequest score() -> mf uses(read); }
    "#,
            ),
            (
                "model",
                r#"
        import "api" as api;
        highkey archetype Founder : api::Scored {
            mf aura;
            init(mf aura) { self.aura = aura; }
            highkey sidequest score() -> mf uses(read) { return self.aura; }
        }
    "#,
            ),
        ],
        "49\n49\n7\n",
    );
}

#[test]
fn imported_function_values_returned_closures_and_lazy_initializers_keep_lexical_scope() {
    run(
        r#"
        import "math" as math;
        main_character {
            fn(mf)->mf uses(abort) increment = math::increment;
            yap(increment(48)); let add = math::adder(7); yap(add(42));
            nonchalant mf answer = math::increment(48); yap(answer); yap(answer);
        }
    "#,
        &[(
            "math",
            r#"
        sidequest helper(mf value) -> mf uses(abort) { return value + 1; }
        highkey sidequest increment(mf value) -> mf uses(abort) { return helper(value); }
        highkey sidequest adder(mf base) -> fn(mf)->mf uses(abort) uses() {
            return value => helper(base + value) - 1;
        }
    "#,
        )],
        "49\n49\n49\n49\n",
    );
}

#[test]
fn imported_helper_graph_specializes_for_scenario_state_without_committing_other_fields() {
    run(
        r#"
        import "model" as model;
        main_character {
            let bro = new model::Founder(20);
            larp (bro) as Trial {
                flex bro.money = 1000; model::increase(bro); canon bro.aura;
            }
            yap(bro.money); yap(bro.aura);
        }
    "#,
        &[(
            "model",
            r#"
        highkey archetype Founder {
            highkey mf money; highkey mf aura;
            init(mf money) { self.money = money; self.aura = 0; }
            highkey sidequest add(mf amount) -> unit uses(read,write,abort) { self.aura += amount; }
        }
        sidequest amount() -> mf uses() { return 49; }
        highkey sidequest increase(Founder bro) -> unit uses(read,write,abort) { bro.add(amount()); }
    "#,
        )],
        "20\n49\n",
    );
}

#[test]
fn private_and_unknown_imported_declarations_are_rejected_at_the_calling_source() {
    for call in ["lib::private()", "lib::missing()", "missing::answer()"] {
        rejects(
            &format!(r#"import "lib" as lib; main_character {{ yap({call}); }}"#),
            &[("lib", "sidequest private() -> mf uses() { return 49; }")],
            &["LARP-N001"],
            SourceId(0),
        );
    }
    rejects(
        r#"import "lib" as lib; main_character { let x = new lib::Secret(); }"#,
        &[("lib", "archetype Secret { init() {} }")],
        &["LARP-T005"],
        SourceId(0),
    );
}

#[test]
fn imported_modules_do_not_reexport_their_import_aliases() {
    rejects(
        r#"import "lib" as lib; main_character { yap(lib::leaf::answer()); }"#,
        &[
            ("lib", "import \"leaf\" as leaf;"),
            (
                "leaf",
                "highkey sidequest answer() -> mf uses() { return 49; }",
            ),
        ],
        &["LARP-N001"],
        SourceId(0),
    );
}

#[test]
fn numeric_arguments_remain_exact_across_imported_calls() {
    rejects(
        r#"import "lib" as lib;
        main_character { unc wide = 49; yap(lib::same(wide)); }"#,
        &[(
            "lib",
            "highkey sidequest same(mf value) -> mf uses() { return value; }",
        )],
        &["LARP-T041", "LARP-T001"],
        SourceId(0),
    );
}

#[test]
fn imported_body_diagnostics_refer_to_the_defining_file() {
    rejects(
        r#"import "lib" as lib; main_character { yap(lib::broken()); }"#,
        &[(
            "lib",
            "highkey sidequest broken() -> mf uses() { return missing; }",
        )],
        &["LARP-N001"],
        SourceId(1),
    );
}

#[test]
fn imported_callbacks_preserve_observe_and_abort_bounds() {
    rejects(
        r#"import "lib" as lib;
        sidequest pure() -> unit uses(abort) { lib::print(49); }
        main_character {}"#,
        &[(
            "lib",
            "highkey sidequest print(mf value) -> unit uses(observe) { yap(value); }",
        )],
        &["LARP-E001"],
        SourceId(0),
    );
    rejects(
        r#"import "lib" as lib;
        sidequest pure() -> unit uses(abort) { mfs<mf> values = [49]; values.forEach(lib::print); }
        main_character {}"#,
        &[(
            "lib",
            "highkey sidequest print(mf value) -> unit uses(observe) { yap(value); }",
        )],
        &["LARP-E001"],
        SourceId(0),
    );
    rejects(
        r#"import "lib" as lib;
        main_character { fn(mf)->mf uses() callback = lib::checked; }"#,
        &[(
            "lib",
            "highkey sidequest checked(mf value) -> mf uses(abort) { return value + 1; }",
        )],
        &["LARP-T001", "LARP-E001"],
        SourceId(0),
    );
}

#[test]
fn lazy_initializers_cannot_hide_imported_observation() {
    rejects(
        r#"import "lib" as lib;
        main_character { nonchalant mf hidden = lib::observe(); yap(hidden); }"#,
        &[(
            "lib",
            "highkey sidequest observe() -> mf uses(observe) { yap(49); return 49; }",
        )],
        &["LARP-E001"],
        SourceId(0),
    );
}

#[test]
fn scenario_specialization_rejects_transitive_io_in_an_imported_helper() {
    rejects(
        r#"import "lib" as lib;
        archetype State { highkey mf value; init() { self.value = 0; } }
        main_character { let state = new State(); larp (state) as Trial { lib::run(); } }"#,
        &[(
            "lib",
            r#"
            highkey sidequest run() -> unit uses(io) { hidden(); }
            sidequest hidden() -> unit uses(io) { let values = args(); }
        "#,
        )],
        &["LARP-E002"],
        SourceId(0),
    );
}

#[test]
fn runtime_failures_in_imported_functions_keep_original_source_spans() {
    let project = project(
        r#"import "lib" as lib; main_character { yap(7); yap(lib::divide(0)); }"#,
        &[(
            "lib",
            "highkey sidequest divide(mf value) -> mf uses(abort) { return 49 / value; }",
        )],
    );
    let compilation = compile(&project);
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let error = larp_vm::execute(
            &compilation.bytecode,
            &mut output,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.code.id, "LARP-R002");
        assert_eq!(error.primary.span.source(), SourceId(1));
        assert_eq!(
            project.sources()[1].slice(error.primary.span),
            Some("49 / value")
        );
        assert_eq!(output, b"7\n");
    }
}

#[test]
fn public_interfaces_cannot_leak_private_nominal_types_at_any_payload_depth() {
    for declaration in [
        "highkey sidequest expose(Secret value) -> unit uses() {}",
        "highkey sidequest expose() -> Secret uses(read,write,abort) { return new Secret(); }",
        "highkey sidequest expose() -> baddie<Secret> uses() { return no_bitches; }",
        "highkey sidequest expose() -> mfs<Secret> uses(abort) { return []; }",
        "highkey sidequest expose() -> hediJeans<Secret,0> uses() { return []; }",
        "highkey sidequest expose() -> result<text,Secret> uses() { return win(\"okay\"); }",
        "highkey sidequest expose(fn(Secret)->mf uses() callback) -> unit uses() {}",
        "highkey sidequest expose() -> fn()->Secret uses(read,write,abort) uses() { return () => new Secret(); }",
        "highkey archetype Public { init(Secret value) {} }",
        "highkey archetype Public { highkey baddie<Secret> value; init() { self.value = no_bitches; } }",
        "highkey archetype Public { init() {} highkey sidequest expose(Secret value) -> unit uses() {} }",
        "highkey role Public { sidequest expose() -> baddie<Secret> uses(); }",
    ] {
        let library = format!("archetype Secret {{ init() {{}} }} {declaration}");
        rejects(
            r#"import "lib" as lib; main_character {}"#,
            &[("lib", &library)],
            &["LARP-M006"],
            SourceId(1),
        );
    }
}

#[test]
fn public_classes_can_encapsulate_private_reference_types_in_private_fields() {
    run(
        r#"import "model" as model;
        main_character { let box = new model::Box(49); yap(box.value()); }"#,
        &[(
            "model",
            r#"
            archetype Secret { highkey mf value; init(mf value) { self.value = value; } }
            highkey archetype Box {
                lowkey Secret secret;
                init(mf value) { self.secret = new Secret(value); }
                highkey sidequest value() -> mf uses(read) { return self.secret.value; }
            }
        "#,
        )],
        "49\n",
    );
}

#[test]
fn identically_named_roles_in_separate_files_remain_nominally_distinct() {
    rejects(
        r#"import "first" as first; import "second" as second;
        main_character { second::Scored value = new first::Founder(); }"#,
        &[
            (
                "first",
                r#"
            highkey role Scored { sidequest score() -> mf uses(); }
            highkey archetype Founder : Scored {
                init() {} highkey sidequest score() -> mf uses() { return 49; }
            }
        "#,
            ),
            (
                "second",
                "highkey role Scored { sidequest score() -> mf uses(); }",
            ),
        ],
        &["LARP-T001"],
        SourceId(0),
    );
}

#[test]
fn imported_helpers_cannot_return_a_scenario_view_or_store_regional_results_outside() {
    let model = r#"
        highkey archetype Counter {
            highkey mf value; init() { self.value = 49; }
        }
        highkey sidequest identity(Counter value) -> Counter uses() { return value; }
        highkey sidequest read(Counter value) -> mf uses(read) { return value.value; }
    "#;
    for (operation, code, source) in [
        ("model::identity(root);", "LARP-E002", SourceId(1)),
        ("outside = model::read(root);", "LARP-S004", SourceId(0)),
    ] {
        rejects(
            &format!(
                r#"import "model" as model;
                main_character {{ let root = new model::Counter(); mf outside = 0;
                    larp (root) as Trial {{ {operation} }} }}"#
            ),
            &[("model", model)],
            &[code],
            source,
        );
    }
}

#[test]
fn qualified_private_function_values_are_rejected_before_closure_creation() {
    rejects(
        r#"import "lib" as lib; main_character { let callback = lib::hidden; }"#,
        &[("lib", "sidequest hidden() -> mf uses() { return 49; }")],
        &["LARP-N001"],
        SourceId(0),
    );
}

#[test]
fn qualified_extensions_select_exact_numeric_receivers_and_contextual_remaining_arguments() {
    run(
        r#"import "ext" as ext;
        main_character {
            mf small = 1; unc wide = 1;
            yap(ext::kind(small)); yap(ext::kind(wide)); yap(ext::kind(1));
            unc total = ext::plus(wide, 48); yap(total);
        }"#,
        &[(
            "ext",
            r#"
            highkey sidequest mf.kind() -> text uses() { return "mf"; }
            highkey sidequest unc.kind() -> text uses() { return "unc"; }
            highkey sidequest unc.plus(unc amount) -> unc uses(abort) { return self + amount; }
        "#,
        )],
        "mf\nunc\nmf\n49\n",
    );
    rejects(
        r#"import "ext" as ext; main_character { unc result = ext::plus(1, 48); }"#,
        &[(
            "ext",
            "highkey sidequest unc.plus(unc amount) -> unc uses(abort) { return self + amount; }",
        )],
        &["LARP-T001"],
        SourceId(0),
    );
}

#[test]
fn selected_extension_results_supply_numeric_context_in_both_operand_orders() {
    run(
        r#"import "ext" as ext;
        main_character {
            unc wide = 3_000_000_000;
            yap(3_000_000_000 + ext::identity(wide));
            yap(ext::identity(wide) + 3_000_000_000);
        }"#,
        &[(
            "ext",
            r#"
            highkey sidequest mf.identity() -> mf uses() { return self; }
            highkey sidequest unc.identity() -> unc uses() { return self; }
        "#,
        )],
        "6000000000\n6000000000\n",
    );
}

#[test]
fn qualified_extension_receiver_is_evaluated_once_before_arguments() {
    run(
        r#"import "ext" as ext;
        sidequest receiver() -> mf uses(observe) { yap("receiver"); return 40; }
        sidequest argument() -> mf uses(observe) { yap("argument"); return 9; }
        main_character { yap(ext::plus(receiver(), argument())); }"#,
        &[(
            "ext",
            "highkey sidequest mf.plus(mf amount) -> mf uses(abort) { return self + amount; }",
        )],
        "receiver\nargument\n49\n",
    );
}

#[test]
fn module_local_dot_extensions_coexist_across_files_and_never_become_implicit_imports() {
    run(
        r#"import "first" as first; import "second" as second;
        sidequest mf.tag() -> text uses() { return "entry"; }
        main_character {
            mf value = 49;
            yap(value.tag()); yap(first::tag(value)); yap(second::tag(value));
            yap(first::local(value)); yap(second::local(value));
        }"#,
        &[
            (
                "first",
                r#"
            highkey sidequest mf.tag() -> text uses() { return "first"; }
            highkey sidequest local(mf value) -> text uses() { return value.tag(); }
        "#,
            ),
            (
                "second",
                r#"
            highkey sidequest mf.tag() -> text uses() { return "second"; }
            highkey sidequest local(mf value) -> text uses() { return value.tag(); }
        "#,
            ),
        ],
        "entry\nfirst\nsecond\nfirst\nsecond\n",
    );
    rejects(
        r#"import "ext" as ext; main_character { mf value = 49; yap(value.tag()); }"#,
        &[(
            "ext",
            "highkey sidequest mf.tag() -> text uses() { return \"imported\"; }",
        )],
        &["LARP-T001"],
        SourceId(0),
    );
}

#[test]
fn real_members_win_dot_lookup_while_qualified_extension_calls_remain_explicit() {
    run(
        r#"import "model" as model; import "ext" as ext;
        sidequest model::Bro.score() -> mf uses() { return 3; }
        main_character {
            let bro = new model::Bro(); yap(bro.score()); yap(ext::score(bro));
        }"#,
        &[
            (
                "model",
                r#"
            highkey archetype Bro { init() {} highkey sidequest score() -> mf uses() { return 1; } }
        "#,
            ),
            (
                "ext",
                r#"
            import "model" as model;
            highkey sidequest model::Bro.score() -> mf uses() { return 2; }
        "#,
            ),
        ],
        "1\n2\n",
    );
}

#[test]
fn qualified_role_extensions_require_the_exact_static_role_receiver() {
    let api = r#"
        highkey role Scored { sidequest score() -> mf uses(); }
        highkey archetype Bro : Scored {
            init() {} highkey sidequest score() -> mf uses() { return 49; }
        }
    "#;
    let extensions = r#"
        import "api" as api;
        highkey sidequest api::Scored.label() -> mf uses() { return self.score(); }
    "#;
    run(
        r#"import "api" as api; import "ext" as ext;
        main_character { api::Scored bro = new api::Bro(); yap(ext::label(bro)); }"#,
        &[("api", api), ("ext", extensions)],
        "49\n",
    );
    rejects(
        r#"import "api" as api; import "ext" as ext;
        main_character { let bro = new api::Bro(); yap(ext::label(bro)); }"#,
        &[("api", api), ("ext", extensions)],
        &["LARP-T001"],
        SourceId(0),
    );
}

#[test]
fn sole_exported_extension_is_a_first_class_unbound_function_with_its_receiver_parameter() {
    run(
        r#"import "ext" as ext;
        main_character {
            let add = ext::plus; yap(add(40, 9));
            fn(mf,mf)->mf uses(abort,observe) wider = ext::plus; yap(wider(48, 1));
        }"#,
        &[(
            "ext",
            "highkey sidequest mf.plus(mf amount) -> mf uses(abort) { return self + amount; }",
        )],
        "49\n49\n",
    );
}

#[test]
fn explicit_function_types_disambiguate_exported_extensions_by_receiver() {
    run(
        r#"import "ext" as ext;
        main_character {
            fn(mf)->text uses(observe) small = ext::kind;
            fn(unc)->text uses() wide = ext::kind;
            yap(small(49)); yap(wide(49));
        }"#,
        &[(
            "ext",
            r#"
            highkey sidequest mf.kind() -> text uses() { return "mf"; }
            highkey sidequest unc.kind() -> text uses() { return "unc"; }
        "#,
        )],
        "mf\nunc\n",
    );
    rejects(
        r#"import "ext" as ext; main_character { let ambiguous = ext::kind; }"#,
        &[(
            "ext",
            r#"
            highkey sidequest mf.kind() -> text uses() { return "mf"; }
            highkey sidequest unc.kind() -> text uses() { return "unc"; }
        "#,
        )],
        &["LARP-M007"],
        SourceId(0),
    );
}

#[test]
fn private_extension_candidates_do_not_leak_through_qualified_dispatch_or_inference() {
    let extensions = r#"
        highkey sidequest mf.kind() -> text uses() { return "public"; }
        lowkey sidequest unc.kind() -> text uses() { return "private"; }
    "#;
    run(
        r#"import "ext" as ext; main_character { let onlyPublic = ext::kind; yap(onlyPublic(49)); }"#,
        &[("ext", extensions)],
        "public\n",
    );
    rejects(
        r#"import "ext" as ext; main_character { unc value = 49; yap(ext::kind(value)); }"#,
        &[("ext", extensions)],
        &["LARP-T001"],
        SourceId(0),
    );
    rejects(
        r#"import "ext" as ext; main_character { fn(unc)->text uses() hidden = ext::kind; }"#,
        &[("ext", extensions)],
        &["LARP-T001"],
        SourceId(0),
    );
}

#[test]
fn exported_free_functions_cannot_share_names_with_exported_extensions() {
    for library in [
        "highkey sidequest tag() -> mf uses() { return 1; } highkey sidequest mf.tag() -> mf uses() { return self; }",
        "highkey sidequest mf.tag() -> mf uses() { return self; } highkey sidequest tag() -> mf uses() { return 1; }",
    ] {
        let project = project(
            r#"import "ext" as ext; main_character {}"#,
            &[("ext", library)],
        );
        let errors = compile_project(&project)
            .err()
            .expect("Expected exported-name collision");
        let error = errors
            .iter()
            .find(|error| error.code.id == "LARP-M007")
            .unwrap_or_else(|| panic!("{}", diagnostics(&project, &errors)));
        assert_eq!(error.primary.span.source(), SourceId(1));
        assert_eq!(project.sources()[1].slice(error.primary.span), Some("tag"));
        assert!(!error.related.is_empty());
        assert_eq!(error.related[0].span.source(), SourceId(1));
        assert_eq!(
            project.sources()[1].slice(error.related[0].span),
            Some("tag")
        );
        assert!(error.related[0].span.range().start < error.primary.span.range().start);
    }
}

#[test]
fn private_free_function_and_exported_extension_may_share_the_same_name() {
    run(
        r#"import "ext" as ext; main_character { yap(ext::tag(49)); yap(ext::local()); }"#,
        &[(
            "ext",
            r#"
            sidequest tag() -> text uses() { return "private free"; }
            highkey sidequest mf.tag() -> text uses() { return "public extension"; }
            highkey sidequest local() -> text uses() { return tag(); }
        "#,
        )],
        "public extension\nprivate free\n",
    );
    run(
        r#"import "ext" as ext; main_character { yap(ext::tag()); yap(ext::local()); }"#,
        &[(
            "ext",
            r#"
            highkey sidequest tag() -> text uses() { return "public free"; }
            sidequest mf.tag() -> text uses() { return "private extension"; }
            highkey sidequest local() -> text uses() { mf value = 49; return value.tag(); }
        "#,
        )],
        "public free\nprivate extension\n",
    );
}

#[test]
fn qualified_extensions_cannot_access_private_fields_even_within_the_declaring_module() {
    rejects(
        r#"import "model" as model; main_character {}"#,
        &[(
            "model",
            r#"
            highkey archetype Bro { mf hidden; init() { self.hidden = 49; } }
            highkey sidequest Bro.expose() -> mf uses(read) { return self.hidden; }
        "#,
        )],
        &["LARP-T010"],
        SourceId(1),
    );
}

#[test]
fn qualified_extension_effects_survive_direct_calls_first_class_values_and_collection_callbacks() {
    let extensions = "highkey sidequest mf.noisy() -> unit uses(observe) { yap(self); }";
    for body in [
        "ext::noisy(49);",
        "let callback = ext::noisy; callback(49);",
        "mfs<mf> values = [49]; values.forEach(ext::noisy);",
    ] {
        rejects(
            &format!(
                r#"import "ext" as ext;
                sidequest pure() -> unit uses(abort) {{ {body} }} main_character {{}}"#
            ),
            &[("ext", extensions)],
            &["LARP-E001"],
            SourceId(0),
        );
    }
    rejects(
        r#"import "ext" as ext; main_character { fn(mf)->unit uses() erased = ext::noisy; }"#,
        &[("ext", extensions)],
        &["LARP-T001"],
        SourceId(0),
    );
}

#[test]
fn qualified_extension_scenario_calls_keep_performed_state_and_selective_commit() {
    run(
        r#"import "model" as model; import "ext" as ext;
        main_character {
            let bro = new model::Bro();
            larp (bro) as Trial { ext::add(bro, 49); canon bro.aura; }
            yap(bro.aura); yap(bro.money);
        }"#,
        &[
            (
                "model",
                r#"
            highkey archetype Bro {
                highkey mf aura; highkey mf money;
                init() { self.aura = 0; self.money = 20; }
            }
        "#,
            ),
            (
                "ext",
                r#"
            import "model" as model;
            highkey sidequest model::Bro.add(mf amount) -> unit uses(read,write,abort) {
                self.aura += amount; self.money = 1000;
            }
        "#,
            ),
        ],
        "49\n20\n",
    );
}

#[test]
fn qualified_extensions_preserve_io_restrictions_through_import_aliases_and_function_values() {
    let extension = r#"
        highkey sidequest mf.input() -> unit uses(io) { let values = args(); }
    "#;
    rejects(
        r#"import "ext" as renamed;
        sidequest pure() -> unit uses() { let callback = renamed::input; callback(49); }
        main_character {}"#,
        &[("ext", extension)],
        &["LARP-E001"],
        SourceId(0),
    );
    rejects(
        r#"import "ext" as renamed;
        archetype State { highkey mf value; init() { self.value = 0; } }
        main_character { let state = new State(); larp (state) as Trial { renamed::input(49); } }"#,
        &[("ext", extension)],
        &["LARP-E002"],
        SourceId(0),
    );
}
