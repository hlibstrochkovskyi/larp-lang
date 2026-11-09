use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const MAIN: &str = "import \"aura.larp\" as aura;\n\nmain_character {\n    mf score = aura::earn(49);\n    yap(\"Your first sidequest:\");\n    yap(score);\n    yap(score mogg 49);\n}\n";

const AURA: &str = "highkey sidequest earn(mf base) -> mf uses(abort) {\n    return base + 1;\n}\n";

pub(super) fn create(name: &str) -> Result<PathBuf, String> {
    validate_name(name)?;
    let parent = std::env::current_dir()
        .map_err(|error| format!("cannot determine the current directory: {error}"))?;
    create_in(&parent, name)
}

pub(super) fn validate_name(name: &str) -> Result<(), String> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 64
        || !bytes[0].is_ascii_alphabetic()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(
            "project name must start with an ASCII letter and contain 1 to 64 ASCII letters, digits, underscores, or hyphens"
                .into(),
        );
    }
    let upper = name.to_ascii_uppercase();
    let device = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && matches!(upper.as_bytes()[3], b'1'..=b'9'));
    if device {
        return Err(format!(
            "project name `{name}` is a reserved Windows device name; choose a portable name"
        ));
    }
    Ok(())
}

fn create_in(parent: &Path, name: &str) -> Result<PathBuf, String> {
    validate_name(name)?;
    let root = parent.join(name);
    // Reserving the destination first prevents replacing an existing project,
    // including a dangling symlink. Later failures leave our partial files intact.
    fs::create_dir(&root).map_err(|error| {
        format!(
            "cannot create project directory `{}`: {error}; the destination must not already exist",
            root.display()
        )
    })?;
    populate(&root, name).map_err(|error| {
        format!(
            "cannot finish project `{}`: {error}; partial files have been preserved",
            root.display()
        )
    })?;
    Ok(root)
}

fn populate(root: &Path, name: &str) -> Result<(), String> {
    let src = root.join("src");
    fs::create_dir(&src).map_err(|error| format!("cannot create `{}`: {error}", src.display()))?;
    let manifest =
        format!("schema = 1\n\n[project]\nname = \"{name}\"\nentry = \"src/main.larp\"\n");
    let readme = format!(
        "# {name}\n\nA LARP project. The entry point imports a small sidequest from `src/aura.larp`.\n\nRun these commands from this directory with the LARP CLI installed:\n\n```sh\nlarp check\nlarp run\nlarp build\n```\n\nThe example prints:\n\n```text\nYour first sidequest:\n50\nfr\n```\n\n`larp.toml` selects `src/main.larp` as the entry point. Edit the source files\nand run `larp check` again before building. Use `larp --help` for CLI options.\n"
    );
    for (relative, content) in [
        ("larp.toml", manifest.as_str()),
        ("src/main.larp", MAIN),
        ("src/aura.larp", AURA),
        ("README.md", readme.as_str()),
        (".gitignore", "/target/\n"),
    ] {
        let path = root.join(relative);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| format!("cannot create `{}`: {error}", path.display()))?;
        file.write_all(content.as_bytes())
            .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "larp-scaffold-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("cannot create fixture: {error}"),
                }
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove owned fixture");
        }
    }

    #[test]
    fn names_are_portable_and_bounded() {
        for name in ["Aura", "my-app", "A_0", "COM0", "lpt10"] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        assert!(validate_name(&"a".repeat(64)).is_ok());
        for name in [
            "", ".", "..", "../app", "a/b", "a\\b", "1app", "-app", "_app", "áura", "a b",
            "app.toml", "a\n", "CON", "con", "pRn", "AUX", "nul", "COM1", "com9", "LPT1", "lpt9",
        ] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
        assert!(validate_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn new_project_contains_only_expected_files_and_no_repository_or_license() {
        let fixture = Fixture::new();
        let root = create_in(&fixture.0, "my-app").expect("create project");
        assert_eq!(root, fixture.0.join("my-app"));
        assert_eq!(
            fs::read_to_string(root.join("larp.toml")).unwrap(),
            "schema = 1\n\n[project]\nname = \"my-app\"\nentry = \"src/main.larp\"\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("src/main.larp")).unwrap(),
            MAIN
        );
        assert_eq!(
            fs::read_to_string(root.join("src/aura.larp")).unwrap(),
            AURA
        );
        assert_eq!(
            fs::read_to_string(root.join(".gitignore")).unwrap(),
            "/target/\n"
        );
        let readme = fs::read_to_string(root.join("README.md")).unwrap();
        for command in ["larp check", "larp run", "larp build"] {
            assert!(readme.contains(command));
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), 4);
        assert_eq!(fs::read_dir(root.join("src")).unwrap().count(), 2);
    }

    #[test]
    fn existing_directories_and_files_are_preserved() {
        let fixture = Fixture::new();
        let directory = fixture.0.join("existing");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("sentinel"), "keep me").unwrap();
        let file = fixture.0.join("another");
        fs::write(&file, "keep this too").unwrap();
        assert!(create_in(&fixture.0, "existing").is_err());
        assert!(create_in(&fixture.0, "another").is_err());
        assert_eq!(
            fs::read_to_string(directory.join("sentinel")).unwrap(),
            "keep me"
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        assert_eq!(fs::read_to_string(file).unwrap(), "keep this too");
    }

    #[test]
    fn invalid_names_do_not_create_files() {
        let fixture = Fixture::new();
        assert!(create_in(&fixture.0, "bad name").is_err());
        assert!(create_in(&fixture.0, "nul").is_err());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn existing_and_dangling_symlinks_are_preserved() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let target = fixture.0.join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("sentinel"), "keep me").unwrap();
        symlink(&target, fixture.0.join("linked")).unwrap();
        let missing = fixture.0.join("missing");
        symlink(&missing, fixture.0.join("dangling")).unwrap();
        assert!(create_in(&fixture.0, "linked").is_err());
        assert!(create_in(&fixture.0, "dangling").is_err());
        assert_eq!(fs::read_link(fixture.0.join("linked")).unwrap(), target);
        assert_eq!(fs::read_link(fixture.0.join("dangling")).unwrap(), missing);
        assert_eq!(
            fs::read_to_string(target.join("sentinel")).unwrap(),
            "keep me"
        );
        assert!(!missing.exists());
    }
}
