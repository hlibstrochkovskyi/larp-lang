# RFC 0002 — Manifest-based project workflow

Status: implemented on `feature/projects` for `0.3.0-dev`; acceptance requires
exact-commit local and hosted checks. See [validation](../validation-project-workflow.md).
The published `v0.2.0-preview.1` does not implement this workflow.

## Contract

A project is a directory containing `larp.toml`, parsed as TOML 1.1:

```toml
schema = 1

[project]
name = "aura-app"
entry = "src/main.larp"
```

All three fields are required. Unknown root/project keys, duplicate keys, wrong
value types, unsupported schema versions, and empty documents are errors. Names
match `[A-Za-z][A-Za-z0-9_-]{0,63}`; Windows device names CON, PRN, AUX, NUL,
COM1..COM9 and LPT1..LPT9 are excluded case-insensitively. Names identify the
output artifact, not an import namespace or package registry identity.

The manifest must be a regular UTF-8 file, at most 64 KiB. TOML syntax diagnostics
retain a source span; schema diagnostics identify their manifest and field.
The canonical manifest parent is the project root, including when the manifest
is selected through a symlink. Its basename must be `larp.toml`.

`entry` is a nonempty relative forward-slash `.larp` path. Absolute paths,
backslashes, NUL, drive prefixes, and lexical traversal above the project root
are errors. The canonical entry must remain inside the root and be a regular
UTF-8 source file. Imports are relative to each importing file. In project mode
all sources must remain inside the manifest root; sibling source directories
are allowed. Existing module count, edge, byte and line limits still apply.
Canonical containment is an ordinary project policy, not protection against a
concurrently hostile filesystem.

## Commands and discovery

- `larp new NAME` reserves a new directory under the current directory and writes
  a manifest, a working two-file program, English README and `.gitignore`.
  Existing files, directories and symlinks are rejected without alteration.
  A later write failure reports the partially created directory; no existing
  user files are deleted. No Git repository or license is implicitly created.
- `larp check`, `larp run [RUN_OPTIONS]`, and `larp build [-o OUTPUT]` discover
  the nearest `larp.toml` in the current directory or its ancestors. Discovery
  stops at the first entry named `larp.toml`, including malformed files, broken
  symlinks and directories. An invalid nearest manifest never falls back to a
  parent project.
- `--manifest-path PATH` after `check`, `run`, or `build` selects that manifest
  explicitly instead of discovery. Options may be in either order before the
  run argument separator. A file operand and `--manifest-path` conflict.
- Existing explicit-file `check FILE`, `run FILE [RUN_OPTIONS]`, and
  `build FILE [-o OUTPUT]` retain their behavior and never discover a manifest.
  Explicit-source imports retain the entry-parent root rule.
- `run` accepts `--fuel U64`, `--gc-stress`, and `-- ARG...` in project mode with
  their existing meaning. Duplicate/unknown/incomplete options are usage errors.
  Project discovery does not change the program's working directory, stdin,
  argument list or host capabilities.
- The default project build output is `ROOT/target/NAME.larpbc`. The CLI creates
  only this default output directory, and rejects a symlink/non-directory at
  `ROOT/target`. A custom `-o` path is relative to the invocation directory and
  its parent must already exist. Compilation failure preserves existing output.
  Output may not replace any loaded source or the selected manifest, including
  canonical aliases. Successful writes use the existing atomic publication path.
- `fmt` and `dump` remain explicit-file commands. There is no implicit recursive
  formatter or project test runner in this RFC.

All command arguments are validated before manifest discovery or execution.
Usage errors exit 2; manifest/filesystem/compiler/runtime failures exit 1;
normal completion exits 0. Manifest errors use `project_error` JSON events in the
existing diagnostics schema, with `LARP-J001` (load/discovery), `LARP-J002`
(TOML/schema), or `LARP-J003` (entry/output policy). Syntax/schema diagnostics
with readable source use the regular structured diagnostic renderer and spans.
Scaffold failures use `project_error` with `LARP-J004`; invalid CLI names exit 2.
Program stdout contains only program output; scaffold success prints its path
and next commands to stdout. Build/check success remains silent.

## Architecture and compatibility

Manifest discovery/schema/scaffolding belong to the CLI. The compiler continues
to accept an injected source resolver; the VM has no knowledge of directories,
manifests or packages. Project commands compile source afresh, never execute a
stale artifact. They preserve the legacy parser for valid standalone entry
syntax while applying the manifest root to source-unit graph resolution.
No artifact wire version, language specification version or diagnostics schema
change is required. Historical artifacts remain executable.

Use the maintained `toml` crate with only `std` and `parse` features. Its selected
transitive source is vendored unchanged, with original licenses and Cargo
checksum metadata, to preserve clean offline builds. This adds no language
package manager, dependency download at runtime, build scripts, registry,
workspaces, multiple targets, re-exports, or separate compilation.

## Acceptance evidence

Execute a generated project through real check/run/build and an independently
loaded artifact, with GC stress and original sources unavailable. Check ancestor
and explicit discovery, an imported sibling directory inside the project root,
source-span errors, stdin/argument/fuel behavior, malformed/duplicate/unknown
manifest fields, limits and path containment, existing-path preservation and
failed-build output preservation. Run the previous single-file/module corpus,
formatter and historical-artifact checks unchanged, plus full workspace lint
and tests. The packaged CLI must execute the project workflow without a Rust
installation or network. A PR records exact local and hosted evidence before
merging; this RFC alone does not prove implementation.

## References

- [TOML crate](https://docs.rs/toml/latest/toml/)
- [Cargo vendoring](https://doc.rust-lang.org/cargo/commands/cargo-vendor.html)
- [Cargo source replacement](https://doc.rust-lang.org/cargo/reference/source-replacement.html)
