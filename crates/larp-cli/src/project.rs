use larp_compiler::project::{ModuleSource, Resolver};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const MAX_SOURCE_BYTES: u64 = 4 * 1024 * 1024;

/// An immutable filesystem snapshot for one source-graph load.
///
/// Canonical path containment is a project policy, not protection against
/// concurrent filesystem replacement or a hostile filesystem.
pub(super) struct FileResolver {
    root: PathBuf,
    sources: HashMap<String, Snapshot>,
}

struct Snapshot {
    name: String,
    text: String,
}

pub(super) fn open_entry(path: &Path) -> Result<(ModuleSource, FileResolver), String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("cannot open entry {}: {error}", path.display()))?;
    let root = canonical
        .parent()
        .ok_or_else(|| "entry source must have a parent directory".to_owned())?
        .to_path_buf();
    let mut resolver = FileResolver {
        root,
        sources: HashMap::new(),
    };
    let source = resolver.snapshot(canonical)?;
    Ok((source, resolver))
}

impl FileResolver {
    fn snapshot(&mut self, canonical: PathBuf) -> Result<ModuleSource, String> {
        if !canonical.starts_with(&self.root) {
            return Err("import resolves outside the entry project's root".to_owned());
        }
        let identity = canonical
            .to_str()
            .ok_or_else(|| "source paths must be valid UTF-8".to_owned())?
            .to_owned();
        if let Some(source) = self.sources.get(&identity) {
            return Ok(ModuleSource {
                identity,
                name: source.name.clone(),
                text: source.text.clone(),
            });
        }
        if !canonical
            .metadata()
            .map_err(|error| format!("cannot inspect source {identity}: {error}"))?
            .is_file()
        {
            return Err(format!("source {identity} is not a regular file"));
        }
        let file = File::open(&canonical)
            .map_err(|error| format!("cannot open source {identity}: {error}"))?;
        let mut bytes = Vec::new();
        file.take(MAX_SOURCE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read source {identity}: {error}"))?;
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(format!(
                "source {identity} exceeds the 4 MiB source-file limit"
            ));
        }
        let text = String::from_utf8(bytes)
            .map_err(|_| format!("source {identity} must contain valid UTF-8"))?;
        self.sources.insert(
            identity.clone(),
            Snapshot {
                name: identity.clone(),
                text: text.clone(),
            },
        );
        Ok(ModuleSource {
            name: identity.clone(),
            identity,
            text,
        })
    }
}

impl Resolver for FileResolver {
    fn resolve(&mut self, importer: &str, request: &str) -> Result<ModuleSource, String> {
        let importer_path = Path::new(importer);
        if !self.sources.contains_key(importer) || !importer_path.starts_with(&self.root) {
            return Err("importer is not part of this project's source snapshot".to_owned());
        }
        let bytes = request.as_bytes();
        let drive_prefix = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        if request.is_empty()
            || !request.ends_with(".larp")
            || request.starts_with('/')
            || request.contains(['\\', '\0'])
            || drive_prefix
            || Path::new(request)
                .extension()
                .is_none_or(|ext| ext != "larp")
        {
            return Err("import paths must be relative .larp paths using '/' without drive prefixes, backslashes, or NUL".to_owned());
        }
        let mut candidate = importer_path
            .parent()
            .ok_or_else(|| "importer must have a parent directory".to_owned())?
            .to_path_buf();
        for component in Path::new(request).components() {
            match component {
                Component::Normal(name) => candidate.push(name),
                Component::CurDir => {}
                Component::ParentDir => {
                    if candidate == self.root || !candidate.pop() {
                        return Err("import path leaves the entry project's root".to_owned());
                    }
                }
                Component::Prefix(_) | Component::RootDir => {
                    return Err("import paths must be relative".to_owned());
                }
            }
        }
        let canonical = candidate
            .canonicalize()
            .map_err(|error| format!("cannot resolve import {request:?}: {error}"))?;
        self.snapshot(canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "larp-file-modules-{}-{}",
                    std::process::id(),
                    NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("cannot create test directory: {error}"),
                }
            }
        }

        fn write(&self, path: &str, text: impl AsRef<[u8]>) -> PathBuf {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn entry_and_relative_library_use_canonical_utf8_identity() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.larp", "main_character {}");
        let library = fixture.write(
            "math.larp",
            "highkey sidequest answer() -> mf { return 42; }",
        );
        let (source, mut resolver) = open_entry(&entry).unwrap();
        assert_eq!(
            source.identity,
            entry.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(source.name, source.identity);
        let loaded = resolver.resolve(&source.identity, "./math.larp").unwrap();
        assert_eq!(
            loaded.identity,
            library.canonicalize().unwrap().to_str().unwrap()
        );
        assert!(loaded.text.contains("return 42"));
    }

    #[test]
    fn nested_imports_resolve_from_importer_and_allow_in_root_parent() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.larp", "main_character {}");
        fixture.write("lib/child.larp", "");
        let shared = fixture.write("shared.larp", "// shared");
        let (source, mut resolver) = open_entry(&entry).unwrap();
        let child = resolver
            .resolve(&source.identity, "lib/child.larp")
            .unwrap();
        let loaded = resolver.resolve(&child.identity, "../shared.larp").unwrap();
        assert_eq!(
            loaded.identity,
            shared.canonicalize().unwrap().to_str().unwrap()
        );
        assert!(
            resolver
                .resolve(&child.identity, "../../outside.larp")
                .is_err()
        );
    }

    #[test]
    fn invalid_requests_and_unknown_importers_are_rejected() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.larp", "main_character {}");
        fixture.write("other.larp", "");
        let (source, mut resolver) = open_entry(&entry).unwrap();
        for request in [
            "",
            "/absolute.larp",
            "C:/file.larp",
            "C:file.larp",
            "lib\\file.larp",
            "file\0.larp",
            "file.txt",
            "file.larp/",
            "../file.larp",
        ] {
            assert!(
                resolver.resolve(&source.identity, request).is_err(),
                "accepted {request:?}"
            );
        }
        let unknown = fixture.0.join("other.larp");
        assert!(
            resolver
                .resolve(unknown.to_str().unwrap(), "main.larp")
                .is_err()
        );
        assert!(
            resolver
                .resolve("/outside/main.larp", "other.larp")
                .is_err()
        );
    }

    #[test]
    fn unreadable_missing_and_invalid_utf8_sources_return_errors() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.larp", "main_character {}");
        fixture.write("invalid.larp", [0xff]);
        std::fs::create_dir(fixture.0.join("directory.larp")).unwrap();
        let (source, mut resolver) = open_entry(&entry).unwrap();
        assert!(resolver.resolve(&source.identity, "missing.larp").is_err());
        assert!(
            resolver
                .resolve(&source.identity, "invalid.larp")
                .unwrap_err()
                .contains("UTF-8")
        );
        assert!(
            resolver
                .resolve(&source.identity, "directory.larp")
                .is_err()
        );
        assert!(open_entry(&fixture.0.join("absent.larp")).is_err());
        assert!(open_entry(&fixture.0.join("invalid.larp")).is_err());
    }

    #[test]
    fn repeated_resolution_returns_the_original_source_snapshot() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.larp", "main_character {}");
        let library = fixture.write("math.larp", "// first");
        let (source, mut resolver) = open_entry(&entry).unwrap();
        let first = resolver.resolve(&source.identity, "math.larp").unwrap();
        std::fs::write(&library, "// changed").unwrap();
        let second = resolver.resolve(&source.identity, "./math.larp").unwrap();
        assert_eq!(first.identity, second.identity);
        assert_eq!(first.name, second.name);
        assert_eq!(first.text, second.text);
        std::fs::write(&entry, "// changed entry").unwrap();
        assert_eq!(
            resolver
                .resolve(&source.identity, "main.larp")
                .unwrap()
                .text,
            source.text
        );
    }

    #[test]
    fn oversized_source_is_rejected_before_full_file_loading() {
        let fixture = Fixture::new();
        let entry = fixture.write("main.larp", "main_character {}");
        let oversized = fixture.write("large.larp", "");
        File::options()
            .write(true)
            .open(oversized)
            .unwrap()
            .set_len(MAX_SOURCE_BYTES + 1)
            .unwrap();
        let (source, mut resolver) = open_entry(&entry).unwrap();
        assert!(
            resolver
                .resolve(&source.identity, "large.larp")
                .unwrap_err()
                .contains("4 MiB")
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_aliases_share_identity_and_outside_targets_are_rejected() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let outside = Fixture::new();
        let entry = fixture.write("main.larp", "main_character {}");
        let library = fixture.write("math.larp", "// original");
        let external = outside.write("external.larp", "// outside");
        symlink(&library, fixture.0.join("alias.larp")).unwrap();
        symlink(external, fixture.0.join("escape.larp")).unwrap();
        let (source, mut resolver) = open_entry(&entry).unwrap();
        let original = resolver.resolve(&source.identity, "math.larp").unwrap();
        let alias = resolver.resolve(&source.identity, "alias.larp").unwrap();
        assert_eq!(original.identity, alias.identity);
        assert_eq!(original.name, alias.name);
        assert_eq!(original.text, alias.text);
        assert!(
            resolver
                .resolve(&source.identity, "escape.larp")
                .unwrap_err()
                .contains("outside")
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_canonical_paths_are_rejected_without_lossy_identity() {
        use std::os::unix::ffi::OsStringExt;
        let fixture = Fixture::new();
        let bad_name = std::ffi::OsString::from_vec(b"bad-\xff.larp".to_vec());
        let path = fixture.0.join(bad_name);
        std::fs::write(&path, "").unwrap();
        assert!(open_entry(&path).err().unwrap().contains("UTF-8"));
    }
}
