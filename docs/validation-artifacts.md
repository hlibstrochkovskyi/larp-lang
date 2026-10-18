# Persisted bytecode validation

Date: 2026-10-18. Scope: the second M6 slice, versioned artifacts and CLI loading.
This is local execution evidence, not a developer-preview release announcement.

## Behavior exercised

`larp build` follows the real frontend/typed IR/bytecode pipeline, encodes an
already verified module, validates the resulting file against the loader policy,
and publishes it by a completed-file rename. `run`, `check`, and bytecode `dump`
accept source or artifacts. Loading independently verifies every module; the VM
still accepts only `VerifiedModule`. Host eval and source translation are absent.

CLI tests build a program containing Unicode text and a fatal assertion, remove
its original source, and compare source/artifact stdout, exit status, and exact
diagnostics. The embedded source location and line survive source deletion.
Repeated builds of the same module/source produce identical bytes. Renaming the
artifact to a nonstandard extension still works through magic detection. Artifact
`check` does not execute observations. `dump --ir` and `build` require source.
Fuel exhaustion and GC stress continue to apply to loaded programs.

Output tests reject direct source overwrite, preserve an existing artifact after
source compilation failure, exercise missing output directories, and check
cleanup of temporary files. Header corruption, unsupported versions, inconsistent
lengths, trailing bytes, and source text disguised with a `.larpbc` extension
fail without executing a program.

## Hostile-input checks

The artifact suite exercises every current opcode, intrinsic, operator, and
scalar tag with roundtrips and stable-tag expectations. All truncations of encoded
operations/intrinsics and a complete small file are rejected. Float tests retain
exact IEEE NaN payloads and negative zero. Unknown tags, Boolean/effect encodings,
UTF-8 errors, oversized counts, missing/prior type references, shared DAGs, source
line limits, invalid source IDs, and invalid UTF-8 span boundaries are covered.
A correctly framed artifact with an invalid entry is rejected by the bytecode
verifier. Stream tests bound oversize input to one sentinel byte, report read
failures, and retry interrupted reads.

Read-only review found two amplification cases and informed concrete regressions:

- A short shared type DAG could repeat a long nominal name many times, causing
  much larger comparisons or error strings than the encoded input. Loader type
  work now charges nominal-name UTF-8 bytes at every expanded occurrence.
- An `ArrayNew` instruction could derive 65,536 complex parameters from one
  encoded element type. Verifier type preflight now shares a module-wide work
  budget that also charges derived parameters, before repeated validation.

The verifier additionally bounds combined parameter/capture/result headers and
processed dataflow cells across functions, including revisits. A multi-function
regression exceeds the shared work limit while each function remains below its
individual stored-state limit. Role implementation lookup uses indexed public
method identities rather than repeatedly scanning the class method list.

See the [format document](bytecode-format-v1.md) for exact wire tables and budgets.
These deterministic malformed-input tests do not replace sustained coverage-guided
fuzzing or measurements of whole-process memory and worst-case verification time.

## Executed validation

The following final local checks passed. CI configuration runs
these checks when hosted; no remote is configured, so hosted CI has not run.

- Workspace formatting and all-targets Clippy with warnings denied.
- 315 workspace tests, including 18 artifact tests and 12 CLI integration tests.
- 117 compiler tests in release mode to retain arithmetic/optimization coverage.
- Corpus inventory validation: 77 active cases, 0 pending.
- All 77 cases through source `check`/`run`, with normal collection and GC stress.
- All 77 cases with `--artifacts`, with normal collection and GC stress. Valid
  programs are built, loaded, checked, and executed; their output/diagnostics must
  equal source execution. Compile-error cases must fail build without creating a
  file. These are 77 cases exercised through additional paths, not new fixtures.
- Git whitespace validation before commit.

The exact commands are in [Development](../CONTRIBUTING.md). The `--artifacts`
path is also configured in CI with GC stress. The format is an internal versioned
preview; it is not a native executable, release archive, package format, or promise
of long-term binary compatibility. Formatting, host APIs, fuzzing, distribution,
platform selection, and a license remain open M6 work.
