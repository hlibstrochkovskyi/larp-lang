# LARP v0.1.0-preview.1

Fake it till you compile.

The first public preview of LARP: an independent, statically typed experimental
language with a Rust compiler and its own bytecode virtual machine. The pipeline
is lexer/parser → name, type and effect analysis → typed control-flow IR →
bytecode → mandatory verifier → VM.

Source commit: [`61cc9314916c510673112dc799e247c0f59cc3b9`](https://github.com/hlibstrochkovskyi/larp-lang/commit/61cc9314916c510673112dc799e247c0f59cc3b9).
License: MIT. This is a prerelease with experimental language semantics.

## Included

- Exact numeric types, contextual literals, explicit checked integer conversions,
  `verdict`, `text`, `mogg`, branches, loops, functions and recursion.
- First-class functions, lambdas with immutable snapshot captures, dynamic and
  fixed arrays, eager collection pipelines and guarded iteration.
- Nominal `archetype` objects, constructors, visibility, methods, roles, concrete
  extensions and bound methods, managed by a tracing garbage collector.
- `baddie<T>` and `result<T,E>`, expected-error recovery, explicit fatal failures,
  and finite effect bounds preserved across indirect calls.
- Experimental single-root `larp` scenarios with `flex`, selective `canon` and
  `palantir`; pure memoized `nonchalant` values and dynamic `sybau` output suppression.
- Explicit capabilities for program arguments and line input.
- CLI source checking/execution, bytecode building/execution, IR inspection,
  a syntax-aware formatter, structured diagnostics and diagnostic explanations.

See the [implemented-language guide](https://github.com/hlibstrochkovskyi/larp-lang/blob/61cc9314916c510673112dc799e247c0f59cc3b9/docs/reference.md)
and [implementation status](https://github.com/hlibstrochkovskyi/larp-lang/blob/61cc9314916c510673112dc799e247c0f59cc3b9/docs/implementation-status.md).
Programs in this preview occupy one source file. Modules are not included.

## Download and run

The binary target is `x86_64-unknown-linux-gnu`. The identical archive was tested
on Ubuntu 24.04 and 26.04. Other platforms are not claimed as supported.
No Rust installation is required to run the extracted binary.

```sh
sha256sum -c larp-0.1.0-preview.1-x86_64-unknown-linux-gnu-61cc9314916c.tar.gz.sha256
tar -xzf larp-0.1.0-preview.1-x86_64-unknown-linux-gnu-61cc9314916c.tar.gz
cd larp-0.1.0-preview.1-x86_64-unknown-linux-gnu-61cc9314916c
sha256sum -c SHA256SUMS
./bin/larp --version --json
./bin/larp run examples/hello.larp
./bin/larp run examples/collections.larp --gc-stress
trial_dir=$(mktemp -d)
./bin/larp build examples/collections.larp -o "$trial_dir/collections.larpbc"
./bin/larp run "$trial_dir/collections.larpbc" --gc-stress
```

The hello example prints `Fake it till you compile.`. The collection example and
its loaded artifact each print `5` and `50` on separate lines.

Archive SHA-256:

```text
4ef6570c24dc929b83cbff2b3aabf921f00719c35d736f0d74139bbebb8b163c
```

The archive contains the executable, complete tracked source, documentation,
examples, test harnesses, MIT license, Rust runtime notices, `BUILD.json` and
payload checksums. GitHub's automatic source archives contain source only.
Checksums identify bytes; this release does not provide cryptographic signatures.

## Validation and compatibility

[Exact-commit CI](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39248451254)
passed formatting, Clippy, 390 workspace tests, 138 release compiler tests,
17 release-tool tests, all 89 conformance cases in four execution modes, and
eight reference samples (40 VM executions, 113 CLI commands).

Two clean Rust 1.99.0 native release builds matched byte for byte. Two package
archives matched byte for byte. The same packaged executable passed both Ubuntu
hosts without rebuilding; the matrix checked matching revision, archive and
binary digests, version identity, and release-document digests.
`release-evidence.json` retains the reports and an additional local preflight
against the extracted source and the downloaded hosted archive.

Toolchain version: `0.1.0-preview.1`; language baseline: experimental `0.1`.
Artifact writer: `2`; readers: `1`, `2`. Diagnostic JSON schema: `1`;
version-info JSON schema: `1`. Genuine frozen historical v1/v2 artifacts were
executed. This is sampled compatibility evidence, not a promise that every
historical artifact is valid.

## Known limits

- `larp` scenarios accept one concrete root with SnapshotValue fields. There is
  no nested scenario, reference-graph isolation, external I/O, hypothetical-value
  escape or outer mutation. Only normal block completion publishes selected
  fields. Fatal failure discards the trial; observations remain visible.
  Selective commit preserves field types and can violate cross-field invariants.
- Default execution limits are 1,000,000 instructions and 1,024 call frames.
  Fuel is not a wall-time limit. There is no VM-wide heap-byte budget, and this
  toolchain is not a sandbox for hostile programs.
- Formatter and embedded source text are limited to 4 MiB; encoded artifacts
  to 16 MiB; logical decoder allocation to 64 MiB. Ordinary source loading does
  not use the formatter's byte cap. Other documented parser/runtime bounds apply.
- Modules/packages, user-defined generics, inheritance, concurrency/async, FFI,
  general host I/O, native code generation and editor services remain deferred.
- Source, CLI, runtime and artifact support may change in later previews.
  Retain source and the previous toolchain, read change notes, and rebuild/test
  applications when updating. No stable public Rust API is promised.

The [technical design review](https://github.com/hlibstrochkovskyi/larp-lang/blob/61cc9314916c510673112dc799e247c0f59cc3b9/docs/preview-design-review.md)
records the numeric, effect, region and selective-commit choices. Finite test
results do not prove the entire language specification. Documentation inside
the immutable archive records the candidate's preparation state before publication.
