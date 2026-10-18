use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "arrays.larp", text)
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
        "{errors:#?}"
    );
    assert!(
        errors.iter().all(|error| error.code.id != "LARP-B001"),
        "frontend failure reached verifier: {errors:#?}"
    );
}

#[test]
fn collection_pipeline_is_eager_ordered_and_allocates_independent_results() {
    run(
        r#"
        main_character {
            mfs<mf> scores = [2, 4, 49];
            let winners = scores.filter(x => x mogg 3).map(x => x + 1).sorted();
            winners.forEach(x => yap(x));
            winners[0] = 100;
            yap(scores[1]);
            mfs<mf> empty = [];
            empty.forEach(x => crashout("empty callback"));
            yap(empty.map(x => x + 1).size());
        }
    "#,
        "5\n50\n4\n0\n",
    );
}

#[test]
fn map_infers_parameters_and_results_and_accepts_context_for_ambiguous_payloads() {
    run(
        r#"
        sidequest double(mf x) -> mf { return x * 2; }
        main_character {
            let input = [1, 2];
            let doubled = input.map(double);
            yap(doubled[1]);
            let optional = input.map(x => some(x));
            yap(optional[0].copium(0));
            mfs<baddie<unc>> missing = input.map(x => no_bitches);
            yap(missing[0].copium(20));
            mfs<mfs<mf>> nested = input.map(x => []);
            nested[0].push(49);
            yap(nested[0][0]);
            let functions = input.map(x => (mf bonus) => x + bonus);
            yap(functions[1](3));
        }
    "#,
        "4\n1\n20\n49\n5\n",
    );
}

#[test]
fn dynamic_arrays_alias_across_calls_captures_and_tagged_values() {
    run(
        r#"
        sidequest append(mfs<mf> values) -> unit uses(write, abort) { values.push(49); }
        main_character {
            let values = [1, 2];
            let alias = values;
            let add = () => values.push(3);
            values = [100];
            add();
            append(alias);
            yap(alias.size());
            yap(values[0]);
            baddie<mfs<mf>> wrapped = some(alias);
            let unwrapped = wrapped.copium([]);
            unwrapped[0] = 7;
            yap(alias[0]);
        }
    "#,
        "4\n100\n7\n",
    );
}

#[test]
fn fixed_arrays_copy_through_nested_values_parameters_and_closures() {
    run(
        r#"
        sidequest change(hediJeans<mf, 2> values) -> mf uses(abort) { values[0] = 99; return values[0]; }
        main_character {
            hediJeans<mf, 2> original = [2, 4];
            let saved = () => original[0];
            let copy = original;
            copy[0] = 49;
            yap(original[0]); yap(copy[0]); yap(change(original));
            original[0] = 7;
            yap(saved());
            hediJeans<hediJeans<mf, 2>, 2> matrix = [[1, 2], [3, 4]];
            let other = matrix;
            matrix[0][1] += 10;
            yap(matrix[0][1]); yap(other[0][1]);
            baddie<hediJeans<mf, 2>> wrapped = some(copy);
            let unwrapped = wrapped.copium([0, 0]);
            unwrapped[0] = 1;
            yap(wrapped.copium([0, 0])[0]);
            hediJeans<mf, 0> empty = [];
            yap(empty.size());
        }
    "#,
        "2\n49\n99\n2\n12\n2\n49\n0\n",
    );
}

#[test]
fn indexed_assignments_save_indices_and_compound_reads_before_the_right_side() {
    run(
        r#"
        sidequest index() -> mf uses(observe) { yap("index"); return 0; }
        sidequest value(mfs<mf> values) -> mf uses(write, observe, abort) { yap("rhs"); values[0] = 100; return 2; }
        sidequest sibling(mfs<hediJeans<mf, 2>> values) -> mf uses(read, write, abort) { values[0][1] = 99; return 7; }
        main_character {
            let values = [10];
            values[index()] += value(values);
            yap(values[0]);
            mfs<hediJeans<mf, 2>> rows = [[1, 2]];
            rows[index()][index()] = sibling(rows);
            yap(rows[0][0]); yap(rows[0][1]);
        }
    "#,
        "index\nrhs\n12\nindex\nindex\n7\n99\n",
    );
}

#[test]
fn fixed_array_values_can_contain_shared_mutable_array_references() {
    run(
        r#"
        main_character {
            let shared = [1];
            hediJeans<mfs<mf>, 1> fixed = [shared];
            let mutate = () => { fixed[0][0] = 7; };
            let copy = fixed;
            mutate();
            yap(copy[0][0]);
            copy[0] = [9];
            yap(fixed[0][0]); yap(copy[0][0]);
        }
    "#,
        "7\n7\n9\n",
    );
}

#[test]
fn sorting_preserves_input_and_text_operations_count_unicode_scalars() {
    run(
        r#"
        main_character {
            let values = [3, -1, 3, 0];
            values.sorted().forEach(x => yap(x));
            yap(values[0]);
            mfs<text> words = ["🦀", "z", "é", "a", "z"];
            words.sorted().forEach(word => yap(word));
            text unicode = "aé🦀";
            yap(unicode.size());
            text combined = "é";
            yap(combined.size());
        }
    "#,
        "-1\n0\n3\n3\n3\na\nz\nz\né\n🦀\n3\n2\n",
    );
}

#[test]
fn callbacks_are_created_before_iteration_and_element_replacement_is_not_structural() {
    run(
        r#"
        sidequest handler() -> fn(mf) -> unit uses(observe) uses(observe) {
            yap("handler"); return x => yap(x);
        }
        sidequest receiver() -> mfs<mf> uses(observe) { yap("receiver"); return [1, 2]; }
        main_character {
            receiver().forEach(handler());
            let values = [1, 2];
            values.forEach(x => { yap(x); values[1] = 9; });
            values.forEach(x => values.forEach(y => no_cap(y > 0)));
            values.push(3);
            yap(values.size());
        }
    "#,
        "receiver\nhandler\n1\n2\n1\n9\n3\n",
    );
}

#[test]
fn collection_effects_distinguish_allocation_value_writes_and_heap_access() {
    run(
        r#"
        sidequest build() -> mfs<mf> uses() { return [1, 2]; }
        sidequest size(hediJeans<mf, 2> values) -> mf uses() { return values.size(); }
        sidequest replace(hediJeans<mf, 2> values) -> mf uses(abort) { values[0] = 49; return values[0]; }
        sidequest map(mfs<mf> values) -> mfs<mf> uses(read) { return values.map(x => x); }
        sidequest set(mfs<mf> values) -> unit uses(write, abort) { values[0] = 7; }
        main_character { let values = build(); set(values); yap(map(values)[0]); yap(size([1, 2])); yap(replace([1, 2])); }
    "#,
        "7\n2\n49\n",
    );
    for text in [
        "sidequest f(mfs<mf> values) -> mf uses() { return values.size(); } main_character {}",
        "sidequest f(mfs<mf> values) -> mf uses(read) { return values[0]; } main_character {}",
        "sidequest f(mfs<mf> values) -> unit uses(abort) { values.push(1); } main_character {}",
        "sidequest f(mfs<mf> values) -> unit uses(write) { values.push(1); } main_character {}",
        "sidequest f(mfs<mf> values) -> unit uses(read) { values.forEach(x => yap(x)); } main_character {}",
        "sidequest f(mfs<mf> values) -> mfs<mf> uses(read) { return values.map(x => x + 1); } main_character {}",
    ] {
        rejects(text, "LARP-E001");
    }
}

#[test]
fn malformed_collection_types_payloads_callbacks_and_places_are_rejected() {
    for (text, code) in [
        ("main_character { let values = []; }", "LARP-T003"),
        ("main_character { mfs<unit> values = []; }", "LARP-T001"),
        (
            "main_character { hediJeans<mf, text> values = []; }",
            "LARP-T001",
        ),
        (
            "main_character { hediJeans<mf, 2147483648> values = []; }",
            "LARP-T004",
        ),
        (
            "main_character { hediJeans<mf, 2> values = [1]; }",
            "LARP-T001",
        ),
        ("main_character { mfs<mf> values = [1, cap]; }", "LARP-T001"),
        (
            "main_character { let values = [1]; unc index = 0; yap(values[index]); }",
            "LARP-T041",
        ),
        (
            "main_character { text value = \"abc\"; yap(value[0]); }",
            "LARP-T001",
        ),
        (
            "main_character { let values = [1.0]; let sorted = values.sorted(); }",
            "LARP-T001",
        ),
        (
            "main_character { let values = [1]; let bad = values.map(x => yap(x)); }",
            "LARP-T001",
        ),
        (
            "main_character { let values = [1]; let bad = values.filter(x => x); }",
            "LARP-T001",
        ),
        (
            "main_character { let values = [1]; values.forEach(x => x); }",
            "LARP-T001",
        ),
        (
            "main_character { let values = [1]; let bad = values.map((unc x) => x); }",
            "LARP-T041",
        ),
        (
            "main_character { let values = [1]; let bad = values.map((mf x, mf y) => x); }",
            "LARP-T006",
        ),
        (
            "main_character { let values = [1]; yap(values == values); }",
            "LARP-T001",
        ),
        (
            "main_character { hediJeans<mf, 1> values = [1]; let f = () => { values[0] = 2; }; }",
            "LARP-T008",
        ),
        (
            "main_character { let values = [1]; values.size(0); }",
            "LARP-T006",
        ),
        (
            "main_character { hediJeans<mf, 1> values = [1]; values.push(2); }",
            "LARP-T001",
        ),
        ("main_character { let value = ascend<3>(1); }", "LARP-T001"),
    ] {
        rejects(text, code);
    }
    for initializer in ["[1]", "() => 1"] {
        let mut text = format!("main_character {{ let value0 = {initializer};");
        for index in 1..140 {
            let expression = if initializer == "[1]" {
                format!("[value{}]", index - 1)
            } else {
                format!("() => value{}", index - 1)
            };
            text.push_str(&format!("let value{index} = {expression};"));
        }
        text.push('}');
        rejects(&text, "LARP-P003");
    }
}

#[test]
fn indexing_and_aliased_structural_mutation_trap_with_source_locations() {
    for (text, code, snippet) in [
        (
            "main_character { let values = [1]; yap(values[-1]); }",
            "LARP-R004",
            "values[-1]",
        ),
        (
            "main_character { hediJeans<mf, 1> values = [1]; yap(values[1]); }",
            "LARP-R004",
            "values[1]",
        ),
        (
            "main_character { let values = [1]; values[1] = 2; }",
            "LARP-R004",
            "values[1]",
        ),
        (
            "main_character { let values = [1]; let alias = values; values.forEach(x => alias.push(x)); }",
            "LARP-R005",
            "alias.push(x)",
        ),
        (
            "main_character { let values = [1]; let alias = values; let bad = values.filter(x => { alias.push(x); return fr; }); }",
            "LARP-R005",
            "alias.push(x)",
        ),
    ] {
        let source = source(text);
        let compiled = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
        for gc_stress in [false, true] {
            let diagnostic = larp_vm::execute(
                &compiled.bytecode,
                &mut Vec::new(),
                larp_vm::Limits {
                    gc_stress,
                    ..Default::default()
                },
            )
            .unwrap_err();
            assert_eq!(diagnostic.code.id, code);
            assert_eq!(source.slice(diagnostic.primary.span), Some(snippet));
        }
    }
}

#[test]
fn invalid_assignment_places_trap_before_observing_or_mutating_the_right_side() {
    let source = source(
        r#"
        sidequest grow(mfs<mf> values) -> mf uses(write, observe, abort) {
            yap("must not run"); values.push(2); return 9;
        }
        main_character { let values = [1]; values[1] = grow(values); }
    "#,
    );
    let compiled = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
    let mut output = Vec::new();
    let diagnostic =
        larp_vm::execute(&compiled.bytecode, &mut output, Default::default()).unwrap_err();
    assert_eq!(diagnostic.code.id, "LARP-R004");
    assert_eq!(source.slice(diagnostic.primary.span), Some("values[1]"));
    assert!(output.is_empty());
}

#[test]
fn fixed_equality_recurses_but_keeps_nested_reference_and_function_restrictions() {
    run(
        r#"
        main_character {
            hediJeans<hediJeans<mf, 2>, 1> first = [[1, 2]];
            hediJeans<hediJeans<mf, 2>, 1> same = [[1, 2]];
            no_cap(first == same);
            same[0][0] = 3;
            no_cap(first != same);
        }
    "#,
        "",
    );
    rejects(
        "main_character { hediJeans<mfs<mf>, 1> x = [[1]]; no_cap(x == x); }",
        "LARP-T001",
    );
}

#[test]
fn fixed_equality_context_types_array_literals_on_either_side() {
    run(
        r#"
        sidequest mark(mf tag) -> mf uses(observe) { yap(tag); return 7; }
        sidequest fixed(mf value) -> hediJeans<mf, 1> uses() { return [value]; }
        main_character {
            mf value = 7;
            hediJeans<mf, 1> expected = [value];
            no_cap([value] == expected);
            no_cap(expected == [value]);
            no_cap([value + 1] != expected);
            no_cap(expected != [value + 1]);
            hediJeans<hediJeans<mf, 1>, 1> nested = [[value]];
            no_cap([[value]] == nested);
            no_cap(nested == [[value]]);
            hediJeans<text, 1> words = ["hello"];
            no_cap(["hello"] == words);
            no_cap(words == ["hello"]);
            hediJeans<mf, 0> empty = [];
            no_cap([] == empty);
            no_cap(empty == []);
            yap([mark(1)] == fixed(mark(2)));
            yap(fixed(mark(3)) == [mark(4)]);
        }
    "#,
        "1\n2\nfr\n3\n4\nfr\n",
    );
}

#[test]
fn fixed_literal_equality_preserves_length_element_and_identity_restrictions() {
    for text in [
        "main_character { hediJeans<mf, 1> x = [1]; no_cap([1, 2] == x); }",
        "main_character { hediJeans<mf, 1> x = [1]; no_cap(x == [1, 2]); }",
        "main_character { mf value = 1; hediJeans<unc, 1> x = [1]; no_cap([value] == x); }",
        "main_character { mf value = 1; hediJeans<unc, 1> x = [1]; no_cap(x == [value]); }",
        "main_character { let x = [1]; no_cap([1] == x); }",
        "main_character { no_cap([1] == [1]); }",
        "main_character { let x = [1]; hediJeans<mf, 1> y = [1]; no_cap(x == y); }",
        "main_character { hediJeans<mfs<mf>, 1> x = [[1]]; no_cap([[1]] == x); }",
    ] {
        rejects(text, "LARP-T001");
    }
}

#[test]
fn source_array_closure_cycles_are_collectible_and_live_map_outputs_survive() {
    let source = source(
        r#"
        sidequest cycle() -> unit uses(write, abort) {
            mfs<fn() -> mf uses(read)> ring = [() => 0];
            ring[0] = () => ring.size();
        }
        main_character {
            let functions = [1, 2, 3].map(x => () => x);
            npc loop (mf i in 0..500) { cycle(); }
            functions.forEach(callback => yap(callback()));
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
    assert_eq!(output, b"1\n2\n3\n");
    assert_eq!(stats.allocations, stats.reclaimed);
    assert!(stats.peak_live_objects < 30, "{stats:?}");
    assert!(stats.collections > 1000);
}
