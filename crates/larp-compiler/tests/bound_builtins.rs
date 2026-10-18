use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "bound-builtins.larp", text)
}
fn run(text: &str, expected: &str) {
    let source = source(text);
    let compiled = compile(&source).unwrap_or_else(|errors| panic!("{errors:#?}"));
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
        assert_eq!(output, expected.as_bytes());
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
        "{errors:#?}"
    );
}

#[test]
fn monomorphic_methods_capture_references_or_fixed_values_once() {
    run(
        r#"
        main_character {
            mfs<mf> values = [3, 1];
            let push = values.push;
            let size = values.size;
            let sorted = values.sorted;
            values = [100];
            push(2); yap(size()); yap(sorted()[0]); yap(values[0]);
            hediJeans<mf,2> fixed = [1, 2];
            let fixedSize = fixed.size;
            fixed[0] = 99;
            yap(fixedSize());
            let textSize = "é🦀".size; yap(textSize());
            baddie<mf> maybe = some(49);
            let present = maybe.isSome; let fallback = maybe.copium;
            maybe = no_bitches;
            yap(present()); yap(fallback(0));
            result<mf,text> ok = win(4);
            let isOk = ok.isOk; ok = fumble("error"); yap(isOk());
        }
    "#,
        "3\n1\n100\n2\n2\nfr\n49\nfr\n",
    );
}

#[test]
fn contextual_callback_methods_preserve_types_order_and_effect_bounds() {
    run(
        r#"
        main_character {
            mfs<mf> values = [3, 1, 2];
            fn(fn(mf) -> mf uses(abort)) -> mfs<mf> uses(read, abort) map = values.map;
            fn(fn(mf) -> verdict uses()) -> mfs<mf> uses(read) filter = values.filter;
            fn(fn(mf) -> unit uses(observe)) -> unit uses(read, observe) each = values.forEach;
            map(x => x + 1).forEach(x => yap(x));
            yap(filter(x => x mogg 1).size());
            each(x => yap(x));
            result<mf,text> failed = fumble("unc");
            fn(fn(text) -> mf uses(observe)) -> mf uses(observe) recover = failed.cope;
            yap(recover(err => { yap(err); return 49; }));
            result<mf,text> success = win(7);
            fn(fn(text) -> mf uses(observe)) -> mf uses(observe) keep = success.cope;
            yap(keep(err => { yap("must not run"); return 0; }));
        }
    "#,
        "4\n2\n3\n2\n3\n1\n2\nunc\n49\n7\n",
    );
}

#[test]
fn extraction_cannot_erase_effects_or_infer_unbound_callback_polymorphism() {
    for method in ["map", "filter", "forEach"] {
        rejects(
            &format!("main_character {{ mfs<mf> xs = [1]; let method = xs.{method}; }}"),
            "LARP-T003",
        );
    }
    rejects(
        "main_character { result<mf,text> r = win(1); let recover = r.cope; }",
        "LARP-T003",
    );
    for text in [
        "main_character { mfs<mf> xs = [1]; fn() -> mf uses() size = xs.size; }",
        "main_character { mfs<mf> xs = [1]; fn(mf) -> unit uses() push = xs.push; }",
        "main_character { mfs<mf> xs = [1]; fn(fn(mf) -> mf uses(observe)) -> mfs<mf> uses(read) map = xs.map; }",
        "main_character { result<mf,text> r = win(1); fn(fn(text) -> mf uses(observe)) -> mf uses() recover = r.cope; }",
        "main_character { result<mf,text> r = win(1); fn(fn(mf) -> mf uses()) -> mf uses() recover = r.cope; }",
        "main_character { mfs<mf> xs = [1]; fn(mf) -> mfs<mf> uses(read) map = xs.map; }",
        "main_character { mfs<verdict> xs = [fr]; let sorted = xs.sorted; }",
    ] {
        rejects(text, "LARP-T001");
    }
    rejects(
        "sidequest bad(mfs<mf> xs) -> mf uses() { let size = xs.size; return size(); } main_character {}",
        "LARP-E001",
    );
}

#[test]
fn extracted_iteration_methods_keep_alias_mutation_guards() {
    let compiled = compile(&source(r#"
        main_character {
            mfs<mf> values = [1];
            let push = values.push;
            fn(fn(mf) -> unit uses(write, abort)) -> unit uses(read, write, abort) each = values.forEach;
            each(x => push(x));
        }
    "#)).unwrap();
    for gc_stress in [false, true] {
        let trap = larp_vm::execute(
            &compiled.bytecode,
            &mut Vec::new(),
            larp_vm::Limits {
                gc_stress,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(trap.code.id, "LARP-R005");
    }
}

#[test]
fn extracted_methods_root_cyclic_receivers_and_returned_function_payloads() {
    run(
        r#"
        sidequest saved() -> fn() -> mf uses(read) uses(read, write, abort) {
            mfs<fn() -> mf uses(read)> callbacks = [];
            let size = callbacks.size;
            callbacks.push(size);
            return callbacks[0];
        }
        main_character {
            let size = saved();
            npc loop (mf i in 0..100) { let garbage = [i]; }
            yap(size());
            result<fn() -> mf uses(read),text> result = win(size);
            fn(fn(text) -> fn() -> mf uses(read) uses()) -> fn() -> mf uses(read) uses() recover = result.cope;
            let get = recover(err => size);
            yap(get());
        }
    "#,
        "1\n1\n",
    );
}
