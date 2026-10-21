# LARP v0.2.0-preview.1 — File Modules

LARP now compiles programs spanning multiple files into one verified bytecode
artifact. This is the second public experimental preview.

## File modules

- Explicit `import "lib/math.larp" as math;` and `math::twice(21)` calls.
- Private-by-default declarations and `highkey` exports.
- Module-qualified classes, roles, functions, and explicit extension calls.
- Lexical name resolution and checked public interfaces, effects, and scenarios.
- One `.larpbc` with embedded source maps; running it needs no original files.
- Library formatting and cross-file diagnostics, including related source labels.

## Download

| Platform | Compiler and VM | Verify | Evidence |
| --- | --- | --- | --- |
| Linux x86-64 GNU | [Download archive](https://github.com/hlibstrochkovskyi/larp-lang/releases/download/v0.2.0-preview.1/larp-0.2.0-preview.1-x86_64-unknown-linux-gnu-a053c991d298.tar.gz) | [SHA-256](https://github.com/hlibstrochkovskyi/larp-lang/releases/download/v0.2.0-preview.1/larp-0.2.0-preview.1-x86_64-unknown-linux-gnu-a053c991d298.tar.gz.sha256) | [Release checks](https://github.com/hlibstrochkovskyi/larp-lang/releases/download/v0.2.0-preview.1/release-evidence.json) |

## Try it

Download the Linux x86-64 GNU archive and matching SHA-256 sidecar. Verify the
sidecar before extraction, then run from the archive's root directory:

```sh
sha256sum -c SHA256SUMS
./bin/larp --version --json
./bin/larp run tests/projects/pitch/main.larp --gc-stress
```

The project prints `49`, `20`, and `fr`. A separate `PALANTIR` report shows
the discarded valuation and selected aura. No Rust installation is required.
The archive includes complete tracked source, documentation, examples, project
fixtures, checksums/provenance, MIT, and the actual Rust runtime notices.

## Compatibility and validation

Toolchain: `0.2.0-preview.1`. Experimental language baseline: `0.1`, extended
by the module RFC. Artifact writer: 2; readers: 1/2. Both JSON schemas remain 1.
Valid legacy single-file programs retain their grammar; `import` becomes
reserved when adopting source-unit grammar. Read the migration notes before
splitting existing programs. There is no stable source or Rust API promise.

The exact candidate passed 480 workspace tests, 193 optimized compiler tests,
19 release-tool tests, the unchanged 89-case corpus in all four modes, reference
and README execution, historical artifact loading, and project execution in five
modes. Two clean builds and packages matched byte-for-byte. The same archive
passed legacy and multi-file execution on Ubuntu 24.04 and 26.04 without rebuilding.

## Limits

No packages, re-exports, cyclic imports, globals/initializers, separate compilation,
user generics, inheritance, async, FFI, native backend, or LSP yet. Imports remain
inside the entry's canonical parent; graph and existing compiler/VM budgets apply.
Scenarios remain non-nested with value-only roots. Selective commit preserves
types, not cross-field application invariants. This preview is not a hostile-code
sandbox or a universal Linux compatibility claim.

MIT License. Copyright (c) 2026 Hlib Strochkovskyi.

[Full notes and migration](https://github.com/hlibstrochkovskyi/larp-lang/blob/a053c991d298e8bf27197561c4df27042b5c7630/docs/releases/0.2.0-preview.1.md)
· [Module guide](https://github.com/hlibstrochkovskyi/larp-lang/blob/a053c991d298e8bf27197561c4df27042b5c7630/docs/modules.md)
· [Passed candidate CI](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39686149176)
· Source: `a053c991d298e8bf27197561c4df27042b5c7630`.
