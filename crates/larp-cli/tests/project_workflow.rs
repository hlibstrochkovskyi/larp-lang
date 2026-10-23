use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "larp-project-workflow-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create test workspace: {error}"),
            }
        }
    }

    fn write(&self, path: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn project(&self, root: &str, name: &str, source: &str) -> PathBuf {
        self.write(
            &format!("{root}/larp.toml"),
            manifest(name, "src/main.larp"),
        );
        self.write(&format!("{root}/src/main.larp"), source);
        self.0.join(root)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn manifest(name: &str, entry: &str) -> String {
    format!("schema = 1\n[project]\nname = \"{name}\"\nentry = \"{entry}\"\n")
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn larp(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_larp"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn with_input(cwd: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_larp"))
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn succeeds(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn silent_success(output: &Output) {
    succeeds(output);
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
}

fn rejects(output: &Output, code: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr.clone()).unwrap();
    assert!(error.contains(code), "{error}");
    error
}

#[test]
fn generated_project_checks_runs_and_builds_a_source_independent_artifact() {
    let workspace = Workspace::new();
    let created = larp(&workspace.0, &["new", "aura-app"]);
    succeeds(&created);
    assert!(created.stderr.is_empty());
    let root = workspace.0.join("aura-app");
    for path in [
        "larp.toml",
        "src/main.larp",
        "src/aura.larp",
        "README.md",
        ".gitignore",
    ] {
        assert!(root.join(path).is_file(), "{path}");
    }
    assert!(!root.join(".git").exists());
    assert!(!root.join("LICENSE").exists());
    silent_success(&larp(&root, &["check"]));
    let expected = larp(&root, &["run"]);
    succeeds(&expected);
    assert_eq!(expected.stdout, b"Your first sidequest:\n50\nfr\n");
    assert!(expected.stderr.is_empty());
    let stressed = larp(&root, &["run", "--gc-stress"]);
    succeeds(&stressed);
    assert_eq!(stressed.stdout, expected.stdout);
    assert_eq!(stressed.stderr, expected.stderr);
    silent_success(&larp(&root, &["build"]));
    let artifact = root.join("target/aura-app.larpbc");
    assert!(artifact.is_file());
    std::fs::rename(root.join("src"), root.join("removed-sources")).unwrap();
    std::fs::rename(root.join("larp.toml"), root.join("removed-manifest")).unwrap();
    for options in [vec![], vec!["--gc-stress"]] {
        let mut args = vec!["run", text(&artifact)];
        args.extend(options);
        let loaded = larp(&workspace.0, &args);
        succeeds(&loaded);
        assert_eq!(loaded.stdout, expected.stdout);
        assert_eq!(loaded.stderr, expected.stderr);
    }
}

#[test]
fn ancestor_discovery_and_manifest_selection_preserve_invocation_directory() {
    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character { yap(49); }");
    let nested = root.join("tools/deep");
    std::fs::create_dir_all(&nested).unwrap();
    silent_success(&larp(&nested, &["check"]));
    silent_success(&larp(&nested, &["build", "-o", "local.larpbc"]));
    assert!(nested.join("local.larpbc").is_file());
    assert!(!root.join("local.larpbc").exists());
    silent_success(&larp(&nested, &["build"]));
    assert!(root.join("target/aura.larpbc").is_file());
    let selected = root.join("larp.toml");
    silent_success(&larp(
        &workspace.0,
        &["check", "--manifest-path", text(&selected)],
    ));
    silent_success(&larp(
        &workspace.0,
        &[
            "build",
            "-o",
            "chosen.larpbc",
            "--manifest-path",
            text(&selected),
        ],
    ));
    assert!(workspace.0.join("chosen.larpbc").is_file());
    assert!(!root.join("chosen.larpbc").exists());
    assert_eq!(larp(&nested, &["run"]).stdout, b"49\n");
}

#[test]
fn nearest_invalid_manifest_never_falls_back_to_a_valid_parent() {
    let workspace = Workspace::new();
    let root = workspace.project("app", "parent", "main_character { yap(49); }");
    let nested = root.join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("larp.toml"), "schema = [").unwrap();
    rejects(&larp(&nested, &["run"]), "LARP-J002");
    let selected = root.join("larp.toml");
    let output = larp(&nested, &["run", "--manifest-path", text(&selected)]);
    succeeds(&output);
    assert_eq!(output.stdout, b"49\n");
}

#[test]
fn manifest_root_allows_sibling_imports_without_changing_explicit_file_rules() {
    let workspace = Workspace::new();
    let root = workspace.project(
        "app",
        "siblings",
        "import \"../lib/math.larp\" as math; main_character { yap(math::answer()); }",
    );
    workspace.write(
        "app/lib/math.larp",
        "highkey sidequest answer() -> mf { return 49; }",
    );
    let output = larp(&root, &["run"]);
    succeeds(&output);
    assert_eq!(output.stdout, b"49\n");
    rejects(&larp(&root, &["check", "src/main.larp"]), "LARP-M001");
    std::fs::write(root.join("larp.toml"), "not valid TOML").unwrap();
    let independent = workspace.write("app/plain.larp", "main_character { yap(20); }");
    let output = larp(&root, &["run", text(&independent)]);
    succeeds(&output);
    assert_eq!(output.stdout, b"20\n");
}

#[test]
fn run_forwards_stdin_arguments_gc_and_fuel_in_both_manifest_option_orders() {
    let workspace = Workspace::new();
    let root = workspace.project(
        "app",
        "input",
        "main_character { args().forEach(x => yap(x)); yap(readLine().cope(err => some(err)).copium(\"EOF\")); }",
    );
    let selected = root.join("larp.toml");
    for args in [
        vec![
            "run",
            "--manifest-path",
            text(&selected),
            "--gc-stress",
            "--fuel",
            "10000",
            "--",
            "🦀",
            "--manifest-path",
            "literal",
        ],
        vec![
            "run",
            "--fuel",
            "10000",
            "--gc-stress",
            "--manifest-path",
            text(&selected),
            "--",
            "🦀",
            "--manifest-path",
            "literal",
        ],
    ] {
        let output = with_input(&workspace.0, &args, b"hello\r\n");
        succeeds(&output);
        assert_eq!(
            output.stdout,
            "🦀\n--manifest-path\nliteral\nhello\n".as_bytes()
        );
        assert!(output.stderr.is_empty());
    }
    rejects(&larp(&root, &["run", "--fuel", "0"]), "LARP-R");
    silent_success(&with_input(&root, &["check"], b"not consumed"));
    silent_success(&with_input(&root, &["build"], b"not consumed"));
}

#[test]
fn invalid_arguments_are_usage_errors_before_manifest_discovery() {
    let workspace = Workspace::new();
    for args in [
        vec!["check", "--manifest-path"],
        vec!["run", "--manifest-path"],
        vec!["build", "--manifest-path"],
        vec!["check", "a.larp", "--manifest-path", "larp.toml"],
        vec!["run", "--manifest-path", "larp.toml", "a.larp"],
        vec!["build", "a.larp", "--manifest-path", "larp.toml"],
        vec!["check", "--manifest-path", "one", "--manifest-path", "two"],
        vec!["run", "--gc-stress", "--gc-stress"],
        vec!["run", "--fuel", "1", "--fuel", "2"],
        vec!["run", "--fuel"],
        vec!["run", "--fuel", "-1"],
        vec!["run", "--unknown"],
        vec!["build", "-o"],
        vec!["build", "-o", "one", "-o", "two"],
        vec!["check", "--gc-stress"],
        vec!["check", "--", "argument"],
        vec!["build", "--gc-stress"],
        vec!["new"],
        vec!["new", "one", "two"],
        vec!["new", "../escape"],
        vec!["new", "NUL"],
    ] {
        let output = larp(&workspace.0, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("LARP-J001"),
            "{args:?}"
        );
    }
    assert_eq!(std::fs::read_dir(&workspace.0).unwrap().count(), 0);
}

#[test]
fn manifest_syntax_and_schema_errors_keep_structured_source_spans() {
    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character {}");
    let selected = root.join("larp.toml").canonicalize().unwrap();
    for source in [
        "# 🦀\nschema = [\n",
        "schema = 1\n[project]\nname = 49\nentry = \"src/main.larp\"\n",
    ] {
        std::fs::write(&selected, source).unwrap();
        let error = rejects(
            &larp(&root, &["--diagnostic-format=json", "check"]),
            "LARP-J002",
        );
        assert_eq!(error.lines().count(), 1, "{error}");
        assert!(error.contains("\"kind\":\"diagnostic\""), "{error}");
        assert!(
            error.contains(&format!("\"path\":{:?}", text(&selected))),
            "{error}"
        );
        assert!(error.contains("\"start\":"), "{error}");
        assert!(error.contains("\"end\":"), "{error}");
        assert!(error.contains("\"source_id\":0"), "{error}");
    }
}

#[test]
fn malformed_schema_unknown_fields_duplicates_and_nonportable_names_are_rejected() {
    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character {}");
    let valid = manifest("aura", "src/main.larp");
    let cases = [
        String::new(),
        "# only a comment\n".into(),
        valid.replace("schema = 1", "schema = 2"),
        valid.replace("schema = 1", "schema = \"1\""),
        valid.replace("schema = 1\n", ""),
        valid.replace("schema = 1", "schema = 1\nschema = 1"),
        valid.replace("schema = 1", "schema = 1\nunknown = true"),
        valid.replace("[project]", "[project]\nunknown = 1"),
        valid.replace("name = \"aura\"", "name = \"aura\"\nname = \"other\""),
        valid.replace("name = \"aura\"\n", ""),
        valid.replace("entry = \"src/main.larp\"\n", ""),
        valid.replace(
            "[project]\nname = \"aura\"\nentry = \"src/main.larp\"",
            "project = []",
        ),
        manifest("CON", "src/main.larp"),
        manifest("../aura", "src/main.larp"),
        manifest(&"a".repeat(65), "src/main.larp"),
    ];
    for source in cases {
        std::fs::write(root.join("larp.toml"), &source).unwrap();
        rejects(&larp(&root, &["check"]), "LARP-J002");
    }
}

#[test]
fn valid_toml_constructs_are_parsed_as_toml_instead_of_line_assignments() {
    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character { yap(49); }");
    for source in [
        "schema = 1\nproject = { name = 'aura', entry = 'src/main.larp' }\n",
        "schema = 1\nproject.name = 'aura'\nproject.entry = '''src/main.larp'''\n",
        "\"schema\" = 1 # supported comments\n[\"project\"]\nname = \"a\\u0075ra\"\nentry = 'src/main.larp'\n",
    ] {
        std::fs::write(root.join("larp.toml"), source).unwrap();
        let output = larp(&root, &["run"]);
        succeeds(&output);
        assert_eq!(output.stdout, b"49\n");
    }
}

#[test]
fn manifest_loading_is_bounded_regular_utf8_and_requires_its_basename() {
    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character {}");
    let selected = root.join("larp.toml");
    let mut boundary = manifest("aura", "src/main.larp");
    boundary.push('#');
    boundary.extend(std::iter::repeat_n('x', 64 * 1024 - boundary.len()));
    std::fs::write(&selected, &boundary).unwrap();
    silent_success(&larp(&root, &["check"]));
    boundary.push('x');
    std::fs::write(&selected, boundary).unwrap();
    rejects(&larp(&root, &["check"]), "LARP-J001");
    std::fs::write(&selected, [0xff]).unwrap();
    rejects(&larp(&root, &["check"]), "LARP-J001");
    let other = workspace.write("other.toml", manifest("aura", "src/main.larp"));
    rejects(
        &larp(&root, &["check", "--manifest-path", text(&other)]),
        "LARP-J001",
    );
    std::fs::rename(&selected, root.join("old-manifest")).unwrap();
    std::fs::create_dir(&selected).unwrap();
    rejects(&larp(&root, &["check"]), "LARP-J001");
}

#[test]
fn entry_policy_rejects_traversal_wrong_extensions_missing_and_non_source_files() {
    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character {}");
    workspace.write("outside.larp", "main_character {}");
    workspace.write("app/src/invalid.larp", [0xff]);
    for entry in [
        "",
        "../outside.larp",
        "src/../../outside.larp",
        "/absolute.larp",
        "C:/absolute.larp",
        "src\\\\main.larp",
        "src/main.txt",
        "src/missing.larp",
        "src/invalid.larp",
    ] {
        std::fs::write(root.join("larp.toml"), manifest("aura", entry)).unwrap();
        rejects(&larp(&root, &["check"]), "LARP-J003");
    }
    std::fs::create_dir(root.join("directory.larp")).unwrap();
    std::fs::write(root.join("larp.toml"), manifest("aura", "directory.larp")).unwrap();
    rejects(&larp(&root, &["check"]), "LARP-J003");
    std::fs::write(
        root.join("larp.toml"),
        manifest("aura", "src/../src/main.larp"),
    )
    .unwrap();
    silent_success(&larp(&root, &["check"]));
}

#[test]
fn builds_preserve_loaded_sources_manifest_and_previous_output_on_failure() {
    let workspace = Workspace::new();
    let root = workspace.project(
        "app",
        "aura",
        "import \"lib.larp\" as lib; main_character { yap(lib::answer()); }",
    );
    workspace.write(
        "app/src/lib.larp",
        "highkey sidequest answer() -> mf { return 49; }",
    );
    for relative in ["larp.toml", "src/main.larp", "src/lib.larp"] {
        let path = root.join(relative);
        let original = std::fs::read(&path).unwrap();
        rejects(&larp(&root, &["build", "-o", text(&path)]), "LARP-J003");
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
    silent_success(&larp(&root, &["build"]));
    let artifact = root.join("target/aura.larpbc");
    let original = std::fs::read(&artifact).unwrap();
    std::fs::write(root.join("src/lib.larp"), "not valid LARP").unwrap();
    let failed = larp(&root, &["build"]);
    assert_eq!(failed.status.code(), Some(1));
    assert_eq!(std::fs::read(&artifact).unwrap(), original);
    let output = larp(&root, &["run", text(&artifact)]);
    succeeds(&output);
    assert_eq!(output.stdout, b"49\n");
    assert_eq!(larp(&root, &["run"]).status.code(), Some(1));
}

#[test]
fn custom_output_parents_are_not_created_and_default_target_must_be_a_directory() {
    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character {}");
    assert_eq!(
        larp(&root, &["build", "-o", "missing/output.larpbc"])
            .status
            .code(),
        Some(1)
    );
    assert!(!root.join("missing").exists());
    std::fs::write(root.join("target"), "preserve me").unwrap();
    rejects(&larp(&root, &["build"]), "LARP-J003");
    assert_eq!(std::fs::read(root.join("target")).unwrap(), b"preserve me");
}

#[test]
fn scaffold_rejects_existing_paths_without_replacing_their_contents() {
    let workspace = Workspace::new();
    workspace.write("existing/sentinel", "keep me");
    workspace.write("file", "keep this too");
    for name in ["existing", "file"] {
        rejects(&larp(&workspace.0, &["new", name]), "LARP-J004");
    }
    assert_eq!(
        std::fs::read(workspace.0.join("existing/sentinel")).unwrap(),
        b"keep me"
    );
    assert_eq!(
        std::fs::read(workspace.0.join("file")).unwrap(),
        b"keep this too"
    );
    assert_eq!(
        std::fs::read_dir(workspace.0.join("existing"))
            .unwrap()
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn symlinked_manifest_uses_its_canonical_parent_and_broken_nearest_links_stop_discovery() {
    use std::os::unix::fs::symlink;

    let workspace = Workspace::new();
    let root = workspace.project("real", "aura", "main_character { yap(49); }");
    let alias = workspace.0.join("alias");
    std::fs::create_dir(&alias).unwrap();
    symlink(root.join("larp.toml"), alias.join("larp.toml")).unwrap();
    let output = larp(&alias, &["run"]);
    succeeds(&output);
    assert_eq!(output.stdout, b"49\n");
    silent_success(&larp(&alias, &["build"]));
    assert!(root.join("target/aura.larpbc").is_file());
    assert!(!alias.join("target").exists());
    let nested = root.join("nested");
    std::fs::create_dir(&nested).unwrap();
    symlink(nested.join("absent"), nested.join("larp.toml")).unwrap();
    rejects(&larp(&nested, &["check"]), "LARP-J001");
}

#[cfg(unix)]
#[test]
fn symlink_entry_and_import_cannot_escape_project_root() {
    use std::os::unix::fs::symlink;

    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character {}");
    let outside = workspace.write("outside.larp", "main_character { yap(49); }");
    symlink(&outside, root.join("src/link.larp")).unwrap();
    std::fs::write(root.join("larp.toml"), manifest("aura", "src/link.larp")).unwrap();
    rejects(&larp(&root, &["check"]), "LARP-J003");
    std::fs::write(root.join("larp.toml"), manifest("aura", "src/main.larp")).unwrap();
    std::fs::write(
        root.join("src/main.larp"),
        "import \"link.larp\" as linked; main_character {}",
    )
    .unwrap();
    rejects(&larp(&root, &["check"]), "LARP-M001");
}

#[cfg(unix)]
#[test]
fn output_aliases_and_symlinked_target_directories_are_rejected_without_changes() {
    use std::os::unix::fs::symlink;

    let workspace = Workspace::new();
    let root = workspace.project("app", "aura", "main_character { yap(49); }");
    for (index, original) in ["larp.toml", "src/main.larp"].iter().enumerate() {
        let target = root.join(original);
        let bytes = std::fs::read(&target).unwrap();
        let link = root.join(format!("alias-{index}"));
        symlink(&target, &link).unwrap();
        rejects(&larp(&root, &["build", "-o", text(&link)]), "LARP-J003");
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        assert_eq!(std::fs::read_link(link).unwrap(), target);
    }
    let output = workspace.0.join("outside-output");
    std::fs::create_dir(&output).unwrap();
    symlink(&output, root.join("target")).unwrap();
    rejects(&larp(&root, &["build"]), "LARP-J003");
    assert_eq!(std::fs::read_dir(output).unwrap().count(), 0);
}

#[test]
fn manifest_entry_limits_apply_even_when_the_legacy_parser_accepts_the_program() {
    let workspace = Workspace::new();
    let source =
        "\n".repeat(larp_compiler::project::MAX_SOURCE_LINES) + "main_character { yap(49); }";
    let root = workspace.project("app", "bounded", &source);
    for args in [vec!["check"], vec!["run"], vec!["build"]] {
        rejects(&larp(&root, &args), "LARP-M005");
    }
    assert!(!root.join("target").exists());
    std::fs::write(
        root.join("src/main.larp"),
        " ".repeat(larp_compiler::project::MAX_SOURCE_BYTES + 1),
    )
    .unwrap();
    rejects(&larp(&root, &["check"]), "LARP-M005");
}

#[test]
fn dash_prefixed_explicit_file_and_output_names_remain_usable() {
    let workspace = Workspace::new();
    workspace.write("-named.larp", "main_character { yap(49); }");
    silent_success(&larp(&workspace.0, &["check", "-named.larp"]));
    let output = larp(&workspace.0, &["run", "-named.larp"]);
    succeeds(&output);
    assert_eq!(output.stdout, b"49\n");
    silent_success(&larp(
        &workspace.0,
        &["build", "-named.larp", "-o", "-named.larpbc"],
    ));
    succeeds(&larp(&workspace.0, &["run", "-named.larpbc"]));
}
