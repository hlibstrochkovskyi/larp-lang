use larp_compiler::compile;
use larp_source::{SourceFile, SourceId};
use larp_vm::{Host, Limits};
use std::io::{BufReader, Cursor};

fn source(text: &str) -> SourceFile {
    SourceFile::new(SourceId(0), "host-input.larp", text)
}
fn compile_ok(source: &SourceFile) -> larp_compiler::Compilation {
    compile(source).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(|error| larp_diagnostics::render(error, source))
                .collect::<String>()
        )
    })
}
fn run(text: &str, arguments: &[&str], input: &[u8], expected: &str) {
    let source = source(text);
    let compilation = compile_ok(&source);
    let arguments: Vec<String> = arguments.iter().map(|value| (*value).into()).collect();
    for gc_stress in [false, true] {
        let mut input = BufReader::with_capacity(1, Cursor::new(input));
        let mut host = Host::new(Some(&arguments), Some(&mut input));
        let mut output = Vec::new();
        let stats = larp_vm::execute_with_host(
            &compilation.bytecode,
            &mut output,
            &mut Vec::new(),
            &mut host,
            Limits {
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
    let source = source(text);
    let errors = compile(&source)
        .err()
        .expect("Expected a checked source rejection");
    assert!(
        errors.iter().any(|error| error.code.id == code),
        "{}",
        errors
            .iter()
            .map(|error| larp_diagnostics::render(error, &source))
            .collect::<String>()
    );
    assert!(errors.iter().all(|error| error.code.id != "LARP-B001" && source.slice(error.primary.span).is_some()));
}

#[test]
fn arguments_are_fresh_arrays_and_zero_parameter_builtin_values_keep_io_effects() {
    run(
        r#"sidequest load()->result<baddie<text>,text> uses(io){return readLine();}
    main_character{
        let fetch=args;
        let first=fetch(); first.push("local");
        no_cap(args().size()==2);
        fetch().forEach(value=>yap(value));
        fn(fn(text)->unit uses(observe))->unit uses(read,observe) saved=first.forEach;
        first=args(); saved(value=>yap(value));
        baddie<text> line=load().cope(err=>some("failed")); yap(line.copium("EOF"));
        let reader=readLine; yap(reader().cope(err=>some(err)).copium("EOF"));
        yap(reader().cope(err=>some(err)).copium("EOF"));
    }"#,
        &["49", "🦀"],
        b"20\r\n\n",
        "49\n🦀\n49\n🦀\nlocal\n20\n\nEOF\n",
    );
}

#[test]
fn stdin_errors_are_result_data_and_do_not_erase_following_record_boundaries() {
    run(
        r#"main_character{
        yap(readLine().cope(err=>some(err)).copium("EOF"));
        yap(readLine().cope(err=>some(err)).copium("EOF"));
        sybau{yap(readLine().cope(err=>some(err)).copium("EOF"));}
        yap(readLine().cope(err=>some(err)).copium("EOF"));
    }"#,
        &[],
        b"\xff\nnext\nconsumed\nlast",
        "input is not valid UTF-8\nnext\nlast\n",
    );
}

#[test]
fn role_bound_methods_extensions_and_callbacks_cannot_hide_host_io() {
    run(
        r#"role Input{sidequest get()->result<baddie<text>,text> uses(io);}
    archetype Reader:Input{init(){}highkey sidequest get()->result<baddie<text>,text> uses(io){return readLine();}}
    sidequest Input.next()->text uses(io){return self.get().cope(err=>some(err)).copium("EOF");}
    main_character{Input input=new Reader();let next=input.next;yap(next());let get=input.get;yap(get().cope(err=>some(err)).copium("EOF"));}
    "#,
        &[],
        b"one\ntwo\n",
        "one\ntwo\n",
    );
    rejects(
        "sidequest pure()->result<baddie<text>,text> uses(){let reader=readLine;return reader();}main_character{}",
        "LARP-E001",
    );
    rejects(
        "main_character{fn()->result<baddie<text>,text> uses() reader=readLine;}",
        "LARP-T001",
    );
    rejects(
        "sidequest pure()->unit uses(){[1].forEach(x=>{readLine();});}main_character{}",
        "LARP-E001",
    );
    rejects(
        "sidequest pure()->unit uses(){sybau{readLine();}}main_character{}",
        "LARP-E001",
    );
    rejects(
        "archetype C{init(){readLine();}}main_character{}",
        "LARP-E001",
    );
    rejects(
        "main_character{nonchalant verdict x=readLine().isOk();}",
        "LARP-E001",
    );
    rejects("main_character{args(1);}", "LARP-T006");
    rejects("main_character{readLine<text>();}", "LARP-P002");
}

#[test]
fn ordinary_prelude_shadowing_does_not_grant_host_input() {
    run(
        "sidequest args()->mf uses(){return 49;}main_character{yap(args());let readLine=()=>20;yap(readLine());}",
        &[],
        b"unused",
        "49\n20\n",
    );
}

#[test]
fn legacy_execution_denies_input_with_a_source_span() {
    for expression in ["args()", "readLine()"] {
        let source = source(&format!("main_character{{{expression};}}"));
        let compilation = compile_ok(&source);
        let mut output = Vec::new();
        let error =
            larp_vm::execute(&compilation.bytecode, &mut output, Limits::default()).unwrap_err();
        assert_eq!(error.code.id, "LARP-R010");
        assert_eq!(source.slice(error.primary.span), Some(expression));
        assert!(output.is_empty());
    }
}

#[test]
fn scenario_graphs_reject_direct_helper_and_indirect_io() {
    let prefix = "archetype State{highkey mf value;init(){self.value=0;}}";
    for body in [
        "readLine();",
        "let read=readLine;read();",
        "helper();",
        "sybau{readLine();}",
        "args();",
    ] {
        rejects(
            &format!(
                "{prefix}sidequest helper()->result<baddie<text>,text> uses(io){{return readLine();}}main_character{{let root=new State();larp(root)as Trial{{{body}}}}}"
            ),
            "LARP-E002",
        );
    }
}

#[test]
fn host_cursor_is_shared_across_executions_and_zero_fuel_never_reads() {
    let source = source("main_character{yap(readLine().cope(err=>some(err)).copium(\"EOF\"));}");
    let compilation = compile_ok(&source);
    let mut cursor = Cursor::new(b"one\ntwo\n");
    let mut host = Host::new(None, Some(&mut cursor));
    let mut output = Vec::new();
    let error = larp_vm::execute_with_host(
        &compilation.bytecode,
        &mut output,
        &mut Vec::new(),
        &mut host,
        Limits {
            instructions: 0,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code.id, "LARP-R006");
    for _ in 0..3 {
        larp_vm::execute_with_host(
            &compilation.bytecode,
            &mut output,
            &mut Vec::new(),
            &mut host,
            Limits::default(),
        )
        .unwrap();
    }
    assert_eq!(output, b"one\ntwo\nEOF\n");
}

#[test]
fn real_input_snapshots_can_be_imported_after_io_finishes_outside_a_scenario() {
    run(
        r#"archetype State{highkey text message;init(){self.message="old";}}
    main_character{
        text input=readLine().cope(err=>some(err)).copium("EOF");
        let state=new State();
        larp(state)as InputSnapshot{state.message=input;canon state.message;}
        yap(state.message);
    }"#,
        &[],
        "🦀\n".as_bytes(),
        "🦀\n",
    );
}
