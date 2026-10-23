# Projects

Status: implemented on the **`0.3.0-dev` development branch**. The current public
release, [`v0.2.0-preview.1`](releases/0.2.0-preview.1-publication.md), supports
[file modules](modules.md) with explicit entry paths but does not provide these
manifest/scaffold commands. Review exact-commit hosted results before merging
or distributing a candidate. This guide describes the implementation; [RFC 0002](rfcs/0002-project-workflow.md)
defines its contract.

## Create and run

Build the development CLI with Rust 1.99.0+ and a native linker:

```sh
cargo build -p larp-cli --release --locked --offline
```

The parser's reviewed dependency sources are vendored; no Cargo registry access
is needed for this build. Add the resulting `target/release` directory to your
`PATH`, or use an absolute path to that executable in place of `larp` below.
A prebuilt development candidate needs neither Rust nor network to run.

From a directory where a new `aura-app` subdirectory may be created:

```sh
larp new aura-app
cd aura-app
larp check
larp run
larp run --gc-stress
larp build
larp run target/aura-app.larpbc --gc-stress
```

`new` creates:

```text
aura-app/
├── larp.toml
├── README.md
├── .gitignore
└── src/
    ├── main.larp
    └── aura.larp
```

The entry imports a public sidequest from `aura.larp`. It prints:

```text
Your first sidequest:
50
fr
```

`check` compiles and verifies without execution. `run` compiles current sources
before executing; it never chooses a stale build artifact automatically. `build`
writes one verified, source-mapped artifact containing all reachable modules.
Running that artifact independently loads and verifies it; original source
files are unnecessary. A successful `check` or `build` is silent.

Names start with an ASCII letter, contain only ASCII letters, digits, `_` or
`-`, and are at most 64 characters. Reserved Windows device names are rejected
case-insensitively. `new` requires a fresh directory name rather than a path:
existing directories, files and symlinks are rejected without alteration. It
does not initialize Git, select a license, install dependencies, or publish
anything. If a later write fails, it reports and preserves its partial directory.

## Manifest

The generated `larp.toml` contains:

```toml
schema = 1

[project]
name = "aura-app"
entry = "src/main.larp"
```

These three fields are required. The CLI parses TOML 1.1 with the maintained
upstream parser and validates a small project schema. Unknown keys, duplicate
keys, wrong types, unsupported schema versions and an empty document are errors.
TOML quoting and escaping follow TOML rules, not LARP string syntax.

`name` follows the same portable naming rules as `new` and determines the default
artifact basename. It is not an import namespace or a registry package identity.
`entry` selects a regular UTF-8 `.larp` source. The manifest itself must be a
regular UTF-8 file of at most 64 KiB. A selected path must be named `larp.toml`.
Its canonical parent is the project root, including when selected via a symlink.

There are no dependency, workspace, target-list, environment-variable or script
fields. Adding unsupported keys does not silently enable behavior.

## Discovery and explicit selection

With no file operand, `check`, `run`, and `build` search the invocation directory
and then its ancestors for the nearest entry named `larp.toml`. Running from
`aura-app/src` therefore selects `aura-app/larp.toml`. The search stops at the
first such entry: a malformed manifest, directory, broken symlink or other load
failure is reported instead of silently selecting a parent project.

Choose a manifest explicitly when invoking from elsewhere:

```sh
larp check --manifest-path aura-app/larp.toml
larp run --manifest-path aura-app/larp.toml --gc-stress
larp run --fuel 100000 --manifest-path aura-app/larp.toml -- hello
larp build --manifest-path aura-app/larp.toml
larp build --manifest-path aura-app/larp.toml -o trial.larpbc
```

`--manifest-path` and other command options may be in either order before `--`.
After the `run` separator, every value is a program argument. `--fuel U64` and
`--gc-stress` have the same behavior as explicit-file execution. Duplicate,
unknown, conflicting and incomplete options are rejected before loading or
running anything. Project selection does not change the invocation working
directory, stdin, program arguments or host capabilities.

Explicit file commands retain their original behavior:

```sh
larp check aura-app/src/main.larp
larp run aura-app/src/main.larp --gc-stress
larp build aura-app/src/main.larp -o trial.larpbc
larp fmt aura-app/src/main.larp --stdout
larp dump trial.larpbc
```

They do not discover a manifest, even inside a project with an invalid one. A
file operand conflicts with `--manifest-path`. `fmt` and `dump` still require
explicit files; there is no recursive project formatter or project test runner.

## Import roots and output

Imports remain relative to the file that contains them. In project mode,
canonical source paths must stay inside the canonical manifest root. For
example, an entry `src/main.larp` may import `../shared/math.larp` when `shared`
is inside the project. Explicit-file mode instead uses the canonical entry
parent as its root, so that same path would escape its boundary and be rejected.
Use project mode when the source layout needs sibling directories.

Entry/import paths use forward slashes and end in `.larp`. Absolute paths,
backslashes, NUL, drive prefixes and traversal above the root are rejected.
Symlinks are resolved before containment is checked. Canonical containment is
an ordinary project path policy, not a guarantee against a concurrently hostile
filesystem. Existing graph limits remain: 64 files, 4096 import edges, 4 MiB
aggregate source bytes, and 131072 lines. Imports execute no initializers.

A project build defaults to `ROOT/target/NAME.larpbc`, independent of the
invocation directory. The CLI creates that default `target` directory when
needed and rejects a symlink or non-directory there. A custom `-o` path is
relative to the invocation directory; its parent must already exist. An
explicit-file build without `-o` continues to replace the entry's extension
with `.larpbc`.

Compilation failure preserves an existing artifact. Output cannot replace any
loaded source or the selected manifest, including canonical aliases. Successful
publication uses the existing atomic file writer. Artifacts embed complete
source text: inspect that content before sharing an artifact publicly.

## Errors and compatibility

Use the global diagnostic option before the command:

```sh
larp --diagnostic-format=json check --manifest-path aura-app/larp.toml
larp explain LARP-J002
```

| Code | Meaning |
| --- | --- |
| `LARP-J001` | Manifest discovery, loading, encoding, or size failure |
| `LARP-J002` | TOML syntax or project schema failure |
| `LARP-J003` | Entry or project output path policy failure |
| `LARP-J004` | Project name or scaffold creation failure |

Readable manifest diagnostics retain source spans; failures without source use
`project_error` events. The existing [diagnostic JSON schema](diagnostics.md)
remains version 1. Invalid CLI usage and invalid scaffold names exit 2; manifest,
filesystem, compilation and runtime failures exit 1; success exits 0. Program
stdout stays separate from diagnostics on stderr. Scaffold creation instructions
are printed to stdout on success.

The workflow belongs to the CLI. It changes no language type/effect rules, VM
host capabilities, artifact wire version or language specification identity.
[Artifact writer 2 and readers 1/2](versioning.md) remain unchanged. Historical
artifact compatibility must still be executed for each candidate, rather than
inferred from those version numbers.

The [CLI acceptance tests](../crates/larp-cli/tests/project_workflow.rs) cover the
implemented boundary; [development checks](development.md) and packaged-candidate
compatibility provide additional evidence. No passing local test establishes
that a later exact commit has passed hosted CI or has been published.
