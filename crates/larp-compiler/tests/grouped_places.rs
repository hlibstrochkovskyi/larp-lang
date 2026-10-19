use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "grouped-places.larp", text)
}

fn run(text: &str, expected: &str) {
    let source = source(text);
    let compiled = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        larp_vm::execute(
            &compiled.bytecode,
            &mut output,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output, expected.as_bytes(), "gc_stress={gc_stress}");
    }
}

#[test]
fn grouped_locals_self_fields_and_fixed_dynamic_indices_execute_as_places() {
    run(
        r#"
        archetype Cell {
            highkey mf value;
            highkey hediJeans<mf, 2> fixed;
            highkey mfs<mf> values;
            init() {
                ((self).value) = 1;
                (self.fixed) = [2, 3];
                ((self).values) = [4, 5];
            }
            highkey sidequest update() -> unit uses(read, write, abort) {
                (((self).value)) += 2;
                ((self.fixed)[1]) *= 3;
                (((self).values)[0]) -= 1;
            }
        }
        main_character {
            mf local = 1;
            (local) = 2;
            ((local)) += 3;
            Cell cell = new Cell();
            ((cell).value) = 10;
            cell.update();
            ((cell.fixed)[0]) += 7;
            ((cell).values)[1] *= 2;
            yap(local);
            yap(cell.value);
            yap(cell.fixed[0]);
            yap(cell.fixed[1]);
            yap(cell.values[0]);
            yap(cell.values[1]);
        }
        "#,
        "5\n12\n9\n9\n3\n10\n",
    );
}

#[test]
fn grouped_places_evaluate_indices_once_before_rhs_and_preserve_sibling_writes() {
    run(
        r#"
        archetype Cell {
            highkey hediJeans<mf, 2> values;
            init() { self.values = [10, 20]; }
        }
        sidequest index(mf marker) -> mf uses(observe) {
            yap(marker);
            return 0;
        }
        sidequest rhs(mfs<Cell> cells) -> mf uses(read, write, observe, abort) {
            yap(3);
            cells[0].values[1] = 99;
            return 5;
        }
        main_character {
            mfs<Cell> cells = [new Cell()];
            (((cells)[index(1)]).values)[index(2)] += rhs(cells);
            yap(cells[0].values[0]);
            yap(cells[0].values[1]);
        }
        "#,
        "1\n2\n3\n15\n99\n",
    );
}

#[test]
fn grouped_invalid_index_aborts_before_rhs_with_the_grouped_place_span() {
    let text = r#"
        sidequest index() -> mf uses(observe) { yap("index"); return 1; }
        sidequest rhs() -> mf uses(observe) { yap("rhs"); return 49; }
        main_character {
            let values = [1];
            ((values)[index()]) = rhs();
        }
    "#;
    let source = source(text);
    let compiled = compile(&source).unwrap();
    for gc_stress in [false, true] {
        let mut output = Vec::new();
        let error = larp_vm::execute(
            &compiled.bytecode,
            &mut output,
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.code.id, "LARP-R004");
        assert_eq!(output, b"index\n");
        assert_eq!(
            source.slice(error.primary.span),
            Some("((values)[index()])")
        );
    }
}

#[test]
fn grouping_does_not_make_temporary_places_or_immutable_bindings_writable() {
    for target in [
        "((1))",
        "([1])[0]",
        "((make()))[0]",
        "((values).make())[0]",
        "((left + right))[0]",
        "((new Cell()).value)",
    ] {
        let errors = compile(&source(&format!("main_character {{ {target} = 2; }}")))
            .err()
            .expect("A temporary-rooted assignment must fail");
        assert!(errors.iter().any(|error| error.code.id == "LARP-P001"));
        assert!(errors.iter().all(|error| error.code.id != "LARP-B001"));
    }
    for text in [
        "main_character { mf x = 1; let f = () => { ((x)) = 2; }; }",
        "main_character { npc loop (mf i in 0..2) { ((i)) += 1; } }",
    ] {
        let errors = compile(&source(text))
            .err()
            .expect("An immutable binding must remain read-only");
        assert!(errors.iter().any(|error| error.code.id == "LARP-T008"));
        assert!(errors.iter().all(|error| error.code.id != "LARP-B001"));
    }
}
