use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_PROJECT: AtomicUsize = AtomicUsize::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "larp-cli-project-{}-{}",
                std::process::id(),
                NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create test project: {error}"),
            }
        }
    }

    fn write(&self, path: &str, text: &str) -> PathBuf {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn larp(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_larp"))
        .args(args)
        .output()
        .unwrap()
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn succeeds(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn rejects(output: &Output, code: &str) -> String {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr.clone()).unwrap();
    assert!(error.contains(code), "{error}");
    error
}

#[test]
fn multifile_source_and_embedded_artifact_execute_through_the_vm() {
    let project = Project::new();
    let entry = project.write(
        "main.larp",
        r#"import "lib/math.larp" as math;
main_character {
    mf math = 7;
    mfs<mf> values = [20, 21];
    mfs<mf> doubled = values.map(x => math::twice(x));
    doubled.forEach(x => yap(x));
    yap(math);
}
"#,
    );
    let library = project.write(
        "lib/math.larp",
        "highkey sidequest twice(mf x) -> mf { return x * 2; }\n",
    );
    let checked = larp(&["check", text(&entry)]);
    succeeds(&checked);
    assert!(checked.stdout.is_empty());
    assert!(checked.stderr.is_empty());
    for flag in [None, Some("--gc-stress")] {
        let mut args = vec!["run", text(&entry)];
        args.extend(flag);
        let output = larp(&args);
        succeeds(&output);
        assert_eq!(output.stdout, b"40\n42\n7\n");
        assert!(output.stderr.is_empty());
    }
    let ir = larp(&["dump", text(&entry), "--ir"]);
    succeeds(&ir);
    assert!(String::from_utf8_lossy(&ir.stdout).contains("Call"));
    let dump = larp(&["dump", text(&entry)]);
    succeeds(&dump);
    assert!(String::from_utf8_lossy(&dump.stdout).contains("Call"));
    let artifact = project.0.join("program.larpbc");
    succeeds(&larp(&["build", text(&entry), "-o", text(&artifact)]));
    std::fs::remove_file(&entry).unwrap();
    std::fs::remove_file(&library).unwrap();
    succeeds(&larp(&["check", text(&artifact)]));
    for flag in [None, Some("--gc-stress")] {
        let mut args = vec!["run", text(&artifact)];
        args.extend(flag);
        let output = larp(&args);
        succeeds(&output);
        assert_eq!(output.stdout, b"40\n42\n7\n");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn artifact_runtime_diagnostics_retain_imported_source_after_sources_are_removed() {
    let project = Project::new();
    let entry = project.write(
        "main.larp",
        "import \"lib.larp\" as lib; main_character { yap(lib::divide(0)); }",
    );
    let library_text = "highkey sidequest divide(mf divisor) -> mf { return 42 / divisor; }";
    let library = project.write("lib.larp", library_text);
    let canonical_library = library.canonicalize().unwrap();
    let artifact = project.0.join("program.larpbc");
    succeeds(&larp(&["build", text(&entry), "-o", text(&artifact)]));
    let original = larp(&["--diagnostic-format=json", "run", text(&entry)]);
    assert_eq!(original.status.code(), Some(1));
    std::fs::remove_file(&entry).unwrap();
    std::fs::remove_file(&library).unwrap();
    for flags in [vec![], vec!["--gc-stress"]] {
        let mut args = vec!["--diagnostic-format=json", "run", text(&artifact)];
        args.extend(flags);
        let output = larp(&args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, original.stderr);
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("\"source_id\":1"), "{error}");
        assert!(
            error.contains(&format!("\"path\":{:?}", text(&canonical_library))),
            "{error}"
        );
        let start = library_text.find("42 / divisor").unwrap();
        assert!(
            error.contains(&format!(
                "\"start\":{start},\"end\":{}",
                start + "42 / divisor".len()
            )),
            "{error}"
        );
        assert!(error.contains("\"text\":\"42 / divisor\""), "{error}");
    }
}

#[test]
fn embedded_sources_resolve_cross_file_related_labels_after_input_removal() {
    let project = Project::new();
    let entry_source = "import \"lib.larp\" as lib; main_character { yap(lib::answer()); }";
    let library_source = "// 🦀\nhighkey sidequest answer() -> mf { return 42; }";
    let entry = project.write("main.larp", entry_source);
    let library = project.write("lib.larp", library_source);
    let names = [
        entry.canonicalize().unwrap(),
        library.canonicalize().unwrap(),
    ];
    let artifact = project.0.join("program.larpbc");
    succeeds(&larp(&["build", text(&entry), "-o", text(&artifact)]));
    std::fs::remove_file(&entry).unwrap();
    std::fs::remove_file(&library).unwrap();
    let loaded = larp_artifact::decode(&std::fs::read(artifact).unwrap()).unwrap();
    let primary_start = library_source.find("42").unwrap();
    let related_start = entry_source.find("lib::answer()").unwrap();
    // Exercise label rendering over loaded maps; this does not add VM stack traces.
    let mut diagnostic = larp_diagnostics::Diagnostic::error(
        "LARP-T001",
        loaded.sources[1]
            .span(primary_start..primary_start + 2)
            .unwrap(),
        "Embedded source-map contract",
    );
    diagnostic.related.push(larp_diagnostics::Label {
        span: loaded.sources[0]
            .span(related_start..related_start + "lib::answer()".len())
            .unwrap(),
        message: "Related entry expression".into(),
    });
    let json = larp_diagnostics::json::render(&diagnostic, &loaded.sources);
    for (id, start, slice) in [
        (1, primary_start, "42"),
        (0, related_start, "lib::answer()"),
    ] {
        assert!(
            json.contains(&format!(
                "\"source_id\":{id},\"path\":{:?},\"start\":{start},\"end\":{}",
                text(&names[id]),
                start + slice.len()
            )),
            "{json}"
        );
        assert!(json.contains(&format!("\"text\":{slice:?}")), "{json}");
    }
    let plain = larp_diagnostics::render_sources(&diagnostic, &loaded.sources);
    assert!(plain.contains(text(&names[0])), "{plain}");
    assert!(plain.contains(text(&names[1])), "{plain}");
}

#[test]
fn imported_type_errors_report_original_utf8_byte_offsets_in_json() {
    let project = Project::new();
    let entry = project.write(
        "main.larp",
        "import \"lib.larp\" as lib; main_character { yap(lib::answer()); }",
    );
    let source =
        "// 🦀 original source\nhighkey sidequest answer() -> mf { unc score = 49; return score; }";
    let library = project.write("lib.larp", source).canonicalize().unwrap();
    let output = larp(&["--diagnostic-format=json", "check", text(&entry)]);
    let error = rejects(&output, "LARP-T041");
    assert_eq!(error.lines().count(), 1, "{error}");
    assert!(error.contains("\"kind\":\"diagnostic\""), "{error}");
    assert!(error.contains("\"source_id\":1"), "{error}");
    assert!(
        error.contains(&format!("\"path\":{:?}", text(&library))),
        "{error}"
    );
    let start = source.rfind("score").unwrap();
    assert!(
        error.contains(&format!("\"start\":{start},\"end\":{}", start + 5)),
        "{error}"
    );
    assert!(error.contains("\"text\":\"score\""), "{error}");
}

#[test]
fn cycle_diagnostics_render_related_sites_in_multiple_source_files() {
    let project = Project::new();
    let entry = project.write("main.larp", "import \"lib.larp\" as lib; main_character {}");
    let library = project.write("lib.larp", "import \"main.larp\" as entry;");
    let output = larp(&["--diagnostic-format=json", "check", text(&entry)]);
    let error = rejects(&output, "LARP-M002");
    assert!(error.contains("\"related\":[{"), "{error}");
    for path in [&entry, &library] {
        assert!(
            error.contains(&format!(
                "\"path\":{:?}",
                text(&path.canonicalize().unwrap())
            )),
            "{error}"
        );
    }
    assert!(!error.contains("\"path\":null"), "{error}");
    for (id, path, end) in [(1, &library, 18), (0, &entry, 17)] {
        assert!(
            error.contains(&format!(
                "\"source_id\":{id},\"path\":{:?},\"start\":7,\"end\":{end}",
                text(&path.canonicalize().unwrap())
            )),
            "{error}"
        );
    }
    let plain = larp(&["check", text(&entry)]);
    let plain = rejects(&plain, "LARP-M002");
    for path in [&entry, &library] {
        assert!(
            plain.contains(text(&path.canonicalize().unwrap())),
            "{plain}"
        );
    }
}

#[test]
fn formatter_accepts_module_entries_and_libraries_without_main() {
    let project = Project::new();
    let entry = project.write(
        "main.larp",
        "import \"lib.larp\" as lib;main_character{yap(lib::answer());}",
    );
    let library = project.write("lib.larp", "highkey sidequest answer()->mf{return 49;}");
    for path in [&entry, &library] {
        let formatted = larp(&["fmt", text(path), "--stdout"]);
        succeeds(&formatted);
        assert!(!formatted.stdout.is_empty());
        succeeds(&larp(&["fmt", text(path)]));
        succeeds(&larp(&["fmt", text(path), "--check"]));
        assert_eq!(std::fs::read(path).unwrap(), formatted.stdout);
    }
    let output = larp(&["run", text(&entry)]);
    succeeds(&output);
    assert_eq!(output.stdout, b"49\n");
}

#[test]
fn module_graph_enforces_entry_counts_and_does_not_hide_unused_dependency_errors() {
    let project = Project::new();
    let entry = project.write("main.larp", "import \"lib.larp\" as lib;");
    let library = project.write("lib.larp", "");
    rejects(&larp(&["check", text(&entry)]), "LARP-M004");
    std::fs::write(&entry, "import \"lib.larp\" as lib; main_character {}").unwrap();
    std::fs::write(&library, "main_character {}").unwrap();
    rejects(&larp(&["check", text(&entry)]), "LARP-M004");
    std::fs::write(
        &library,
        "highkey sidequest invalid() -> mf { return cap; }",
    )
    .unwrap();
    rejects(&larp(&["check", text(&entry)]), "LARP-T001");
}

#[test]
fn imported_calls_preserve_privacy_numeric_types_and_effect_bounds() {
    let project = Project::new();
    let library = project.write("lib.larp", "sidequest hidden() -> mf { return 49; }");
    let entry = project.write(
        "main.larp",
        "import \"lib.larp\" as lib; main_character { yap(lib::hidden()); }",
    );
    rejects(&larp(&["check", text(&entry)]), "LARP-N001");
    std::fs::write(&library, "highkey sidequest answer() -> unc { return 49; }").unwrap();
    std::fs::write(
        &entry,
        "import \"lib.larp\" as lib; main_character { mf score = lib::answer(); }",
    )
    .unwrap();
    rejects(&larp(&["check", text(&entry)]), "LARP-T041");
    std::fs::write(
        &library,
        "highkey sidequest observe() -> mf uses(observe) { yap(49); return 49; }",
    )
    .unwrap();
    std::fs::write(&entry, "import \"lib.larp\" as lib; sidequest pure() -> mf uses() { return lib::observe(); } main_character { yap(pure()); }").unwrap();
    rejects(&larp(&["check", text(&entry)]), "LARP-E001");
}

#[test]
fn artifact_output_cannot_overwrite_an_imported_source() {
    let project = Project::new();
    let entry_text = "import \"lib.larp\" as lib; main_character { yap(lib::answer()); }";
    let library_text = "highkey sidequest answer() -> mf { return 49; }";
    let entry = project.write("main.larp", entry_text);
    let library = project.write("lib.larp", library_text);
    let output = larp(&["build", text(&entry), "-o", text(&library)]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must differ"));
    assert_eq!(std::fs::read_to_string(&entry).unwrap(), entry_text);
    assert_eq!(std::fs::read_to_string(&library).unwrap(), library_text);
}

#[test]
fn imports_reject_external_paths_and_unavailable_sources() {
    let project = Project::new();
    let outside = Project::new();
    let external = outside.write(
        "external.larp",
        "highkey sidequest answer() -> mf { return 49; }",
    );
    let entry = project.write(
        "main.larp",
        &format!("import {:?} as lib; main_character {{}}", text(&external)),
    );
    rejects(&larp(&["check", text(&entry)]), "LARP-M001");
    std::fs::write(&entry, "import \"missing.larp\" as lib; main_character {}").unwrap();
    rejects(&larp(&["check", text(&entry)]), "LARP-M001");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&external, project.0.join("escape.larp")).unwrap();
        std::fs::write(&entry, "import \"escape.larp\" as lib; main_character {}").unwrap();
        rejects(&larp(&["check", text(&entry)]), "LARP-M001");
    }
}

#[test]
fn legacy_import_identifier_remains_valid_for_run_and_format() {
    let project = Project::new();
    let entry = project.write(
        "legacy.larp",
        "sidequest import(mf x)->mf{return x+1;}main_character{yap(import(48));}",
    );
    succeeds(&larp(&["fmt", text(&entry)]));
    succeeds(&larp(&["fmt", text(&entry), "--check"]));
    let output = larp(&["run", text(&entry)]);
    succeeds(&output);
    assert_eq!(output.stdout, b"49\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn committed_pitch_project_executes_imported_extensions_and_selective_commit() {
    let project = Project::new();
    let units = [
        (
            "main.larp",
            include_str!("../../../tests/projects/pitch/main.larp"),
        ),
        (
            "lib/founder.larp",
            include_str!("../../../tests/projects/pitch/lib/founder.larp"),
        ),
        (
            "lib/training.larp",
            include_str!("../../../tests/projects/pitch/lib/training.larp"),
        ),
    ];
    let paths: Vec<_> = units
        .iter()
        .map(|(name, source)| project.write(name, source))
        .collect();
    let entry = &paths[0];
    for path in &paths {
        succeeds(&larp(&["fmt", text(path), "--check"]));
        let formatted = larp(&["fmt", text(path), "--stdout"]);
        succeeds(&formatted);
        assert_eq!(formatted.stdout, std::fs::read(path).unwrap());
        succeeds(&larp(&["fmt", text(path)]));
        let second = larp(&["fmt", text(path), "--stdout"]);
        succeeds(&second);
        assert_eq!(second.stdout, formatted.stdout);
    }
    let checked = larp(&["check", text(entry)]);
    succeeds(&checked);
    assert!(checked.stdout.is_empty());
    assert!(checked.stderr.is_empty());
    let expected_inspection = b"PALANTIR [CrossFilePitch]\nPROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED\nvaluation | 49 | 1000000000 | 1000000000 | false\naura | 0 | 20 | - | true\n";
    for stress in [false, true] {
        let mut args = vec!["run", text(entry)];
        if stress {
            args.push("--gc-stress");
        }
        let output = larp(&args);
        succeeds(&output);
        assert_eq!(output.stdout, b"49\n20\nfr\n");
        assert_eq!(output.stderr, expected_inspection);
    }
    let artifact = project.0.join("pitch.larpbc");
    succeeds(&larp(&["build", text(entry), "-o", text(&artifact)]));
    for path in paths {
        std::fs::remove_file(path).unwrap();
    }
    succeeds(&larp(&["check", text(&artifact)]));
    for stress in [false, true] {
        let mut args = vec!["run", text(&artifact)];
        if stress {
            args.push("--gc-stress");
        }
        let output = larp(&args);
        succeeds(&output);
        assert_eq!(output.stdout, b"49\n20\nfr\n");
        assert_eq!(output.stderr, expected_inspection);
    }
}
