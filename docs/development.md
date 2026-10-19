# Engineering guide

Start with [Contributing](../CONTRIBUTING.md) for setup and the required local
checks. This guide covers additional verification and the contracts most likely
to be affected by compiler, runtime, or tooling work.

## Find the relevant code

The [architecture](architecture.md) describes the compiler pipeline and
[crate boundaries](architecture.md#workspace-boundaries). The
[implementation status](implementation-status.md) distinguishes working features
from deferred ones. The [roadmap](roadmap.md) organizes work into executable
acceptance gates; it is not a schedule or blanket approval of the proposed
[language specification](spec/v0.1.md).

Source follows this production path:

```text
source → lexer/parser → name/type/effect checking → typed control-flow IR
       → bytecode → independent verification → LARP VM
```

Persisted artifacts pass a bounded decoder before verification. The production
implementation does not translate programs into another language or use host
`eval`. Add crates when a boundary has working code, rather than creating empty
subsystems in anticipation of future work.

## Additional checks

The required local baseline is in [Contributing](../CONTRIBUTING.md#check-your-change).
Use the checks below for the affected behavior. The
[CI workflow](../.github/workflows/ci.yml) is the authoritative list of automated
gates; bounded fuzz campaigns have a [separate workflow](../.github/workflows/fuzz.yml).

| Change | Relevant additional verification |
| --- | --- |
| Compiler control flow or checked arithmetic | Optimized compiler tests |
| VM values, heap roots, closures, or callbacks | GC-stress conformance |
| Artifact encoder, loader, or verifier | Loaded-artifact conformance and version compatibility |
| Formatter or parser layout | Formatted/loaded-artifact conformance |
| README or practical reference examples | Documentation execution harness and source consistency |
| CLI identity, release metadata, or candidate tooling | Version-contract check and candidate-tool unit tests |
| Runtime resource accounting | Resource measurements on Linux |
| Build or distribution tooling | Independent reproducibility and packaged-candidate checks |

```sh
cargo test -p larp-compiler --release --locked --offline
python3 tools/run_conformance.py --gc-stress
python3 tools/run_conformance.py --artifacts --gc-stress
python3 tools/run_conformance.py --formatted --artifacts --gc-stress
python3 tools/run_reference.py --output artifacts/reference-my-change
python3 tools/run_reference.py --document README.md --contracts docs/readme-cases.json --output artifacts/readme-my-change
python3 tools/check_version.py --bin target/debug/larp --output artifacts/version-my-change
python3 -m unittest discover -s tools -p 'test_candidate_checks.py'
```

Build the current CLI before supplying it to `check_version.py`; that tool never
rebuilds its `--bin`. Use new output paths for repeated evidence runs. The
reference and version harnesses reject nonempty output directories so previous
results remain available.

Conformance `--gc-stress` collects at every object/array/closure allocation
safepoint, including lazy cells. `--artifacts` builds valid source, loads and
verifies it, and compares execution; rejected source must not leave an artifact.
`--formatted` checks temporary formatted copies for idempotence and executes the
same contracts. These modes compose, and do not rewrite checked-in fixtures.
`--all` additionally attempts pending cases if any exist.

The [reference harness](../tools/run_reference.py) extracts every LARP fence in
[the practical reference](reference.md) and checks the ordered contracts in
[reference-cases.json](reference-cases.json) through source execution, formatting,
and loaded artifacts, with normal and stress collection. Update the contract
when changing a sample. Do not substitute a syntax-only check for execution.

The README has its own [execution contract](readme-cases.json). Its LARP fence
must also match [examples/pitch-deck.larp](../examples/pitch-deck.larp) exactly,
so the front page cannot silently drift from the downloadable source example.
CI executes both documents. `--bin PATH` checks an existing CLI without rebuilding
it; use that option when verifying a sample against a published preview.

For measurements or distribution changes, follow the prerequisites and evidence
rules in these guides:

- [Resource measurement](validation-resources.md): Linux and `cc` are required;
  traced allocation counts are not total process memory or a heap-byte budget.
- [Reproducible builds](reproducible-builds.md): the driver builds a tracked commit
  twice, not uncommitted worktree changes.
- [Distribution](distribution.md) and [platform policy](platforms.md): candidate
  archives include source and license notices; compatibility executes the same
  archive on each supported host.
- [Fuzz workspace](../fuzz/README.md): pinned tooling, separate dependencies,
  bounded campaigns, and public inputs. Do not disable production verification
  or effect checks to run a target.

## Contracts to preserve

**Locations and diagnostics.** Retain source IDs and half-open UTF-8 byte spans
through every phase, including persisted source maps. Emit structured diagnostics
with stable codes; see the [diagnostic protocol](diagnostics.md). Never replace
errors with jokes that obscure the cause or the affected source.

**Types and effects.** Preserve exact numeric widths and checked integer behavior
in debug and optimized builds. Visibility, effect, and region restrictions must
hold through aliases, callbacks, bound methods, role dispatch, and loaded
bytecode. Follow the rules in the [specification](spec/v0.1.md) and the
[architecture](architecture.md#closures-effects-and-result-dispatch).

**Objects and collection.** Constructor initialization must publish only complete
objects; partial `self` cannot escape. Preserve member visibility, immutable
`self` bindings, and lexical private access in closures. Trace temporary values,
callback receivers and unfinished outputs, active lazy cells, and transitive
object/array/closure cycles. Tests should distinguish live cycles from unreachable
cycles and exercise generation reuse. See
[object validation](validation-objects.md) and
[role/extension validation](validation-roles-and-extensions.md).

**Alternative state.** Keep the regional call graph closed and route view reads
and writes to performed state. Outer snapshots are immutable; only selected final
fields publish, together, on normal fallthrough. Traps discard performed state,
while already emitted output remains. The implementation is experimental and
value-only: selective commit preserves field types, not application invariants.
See [scenario validation](validation-scenarios.md) and
[scenario evaluation](scenario-evaluation.md). The isolated Rust model in
`experiments/scenario-model` does not prove compiler region typing, GC, or
scenario bytecode behavior.

**Laziness and suppression.** Lazy initializers capture SnapshotValues at
declaration and memoize on first use. Capturing an existing lazy binding forces
it immediately, including in an ordinary closure; preserve its abort bound.
Lazy cells cannot escape into source-level fields, captures, or containers.
`sybau` dynamically mutes only ordinary `yap`; it does not remove effects, mute
inspection, or hide diagnostics. Scope exits restore the caller's suppression
depth. See [lazy/suppression validation](validation-lazy-and-suppression.md).

**Host input.** Static `io` bounds and runtime capabilities are separate contracts.
Legacy embedding APIs deny input. Preserve EOF/blank-line distinctions, record
boundaries after UTF-8 errors, failure latching, input budgets, regional guards,
and roots across argument-array allocation. See [host input](host-input.md).

**Formatting and artifacts.** Formatting must preserve raw token/comment order
and parentheses, then reparse and compare lexical streams before publishing a
result. Artifact loading must enforce shared budgets and independent verification
before handing a module to the VM. Source and artifact writes publish complete
sibling temporary files by rename. See [formatter policy](formatter.md) and
[bytecode format v2](bytecode-format-v2.md).

## Keep evidence honest

Use small vertical slices with executable acceptance criteria. A fixture becomes
an implemented example only after the real compiler and VM execute it. Keep
status documents current and label historical validation reports with their
original revisions and limits.

Generated evidence lives under ignored `artifacts/`. Keep previous runs, choose
fresh output paths, and do not commit build trees or private notes. The private
`Ideas-not-for push!/` directory is not a source for public files or fuzz inputs.
VM fuel limits instructions, not wall-clock time or host allocations; managed
GC does not promise recovery from host allocation failure.
