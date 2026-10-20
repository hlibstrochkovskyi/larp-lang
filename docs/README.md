# LARP documentation

Start with the [practical reference](reference.md) to write a program, or
[CONTRIBUTING](../CONTRIBUTING.md) to work on the compiler.
[`v0.1.0-preview.1`](releases/0.1.0-preview.1-publication.md) is the first public
experimental release. The implementation status describes what works; the
specification describes the design baseline. Neither a proposal nor a passing
finite test suite establishes complete language correctness.

Current development: **`0.2.0-dev`**, including [file modules](modules.md);
imports are not available in the published first preview.

## Learn and run

| Guide | Use it for |
| --- | --- |
| [Practical reference](reference.md) | Types, control flow, functions, objects, effects, and complete runnable examples |
| [File modules](modules.md) | Imports, exports, lexical namespaces, extensions, and whole-project artifacts |
| [Example programs](../examples/) | Collections, roles, lazy values, input, and scenario comparisons |
| [Implementation status](implementation-status.md) | Supported syntax, CLI commands, restrictions, and runtime limits |
| [Diagnostics](diagnostics.md) | Diagnostic codes, source locations, JSON Lines, and inspection output |
| [Formatter](formatter.md) | Preview, check, and write modes; token/comment preservation |
| [Host input](host-input.md) | Program arguments, stdin, EOF, expected errors, and embedding capabilities |
| [Scenario evaluation](scenario-evaluation.md) | What selective commit helps with, explicit-copy alternatives, and invariant limits |

## Develop the toolchain

| Guide | Use it for |
| --- | --- |
| [Contributing](../CONTRIBUTING.md) | Checkout, first run, bug reports, focused PRs, and baseline checks |
| [Development](development.md) | Additional validation and semantic contracts to preserve |
| [Architecture](architecture.md) | Compiler phases, crate responsibilities, verifier, GC, and runtime boundaries |
| [Roadmap](roadmap.md) | Completed milestones and the next language/tooling work |
| [Conformance corpus](../tests/conformance/README.md) | Positive, compile-fail, and runtime-error contracts |
| [Fuzzing](../fuzz/README.md) | Separate instrumented workspace and stable regression replay |
| [State-model experiment](../experiments/scenario-model/README.md) | Independent Rust transition model and its limited scope |

Normal clones use ordinary Git. [Git setup](git-setup.md) documents an alternate
metadata directory used only in the original managed checkout; it is not a
contributor prerequisite.

## Language and runtime contracts

- [Specification v0.1](spec/v0.1.md) and [grammar](spec/grammar.ebnf).
- [Design review](design-review.md), [preview policy review](preview-design-review.md),
  and [research sources](research.md).
- [Bytecode v1](bytecode-format-v1.md) and [v2 input additions](bytecode-format-v2.md).
- [Version identities and compatibility](versioning.md), including independent
  artifact and JSON schemas.

## Releases and distribution

- [First preview publication](releases/0.1.0-preview.1-publication.md): exact
  source commit, downloadable binary, checksums, and hosted evidence.
- [Frozen first-preview candidate notes](releases/0.1.0-preview.1.md): the
  preparation document embedded in that immutable archive.
- [Distribution](distribution.md): package contents, notices, provenance, and checks.
- [Platforms](platforms.md): supported build/execution hosts and the same-archive gate.
- [Reproducible builds](reproducible-builds.md): two clean builds and the scope of
  the byte-equality claim.
- [Preview readiness audit](preview-readiness.md): requirement evidence, resolved
  findings, and remaining broader design work.

## Validation records

These documents record particular executions and findings. Their dates and
source revisions matter; an old passing result does not validate a newer commit.

| Area | Records |
| --- | --- |
| Foundation and algorithms | [Foundation](validation-foundation.md), [scalars](validation-scalar-core.md), [functions/recovery](validation-functions-and-recovery.md) |
| Types and control flow | [Contextual equality](validation-contextual-equality.md), [constant control flow](validation-control-flow.md) |
| File modules | [Whole-project compilation](validation-file-modules.md) |
| Collections and objects | [Collections](validation-collections.md), [objects](validation-objects.md), [roles/extensions](validation-roles-and-extensions.md) |
| Alternative state | [Scenarios](validation-scenarios.md), [lazy/suppression](validation-lazy-and-suppression.md) |
| CLI and persisted programs | [Artifacts](validation-artifacts.md), [formatter](validation-formatter.md), [input](validation-host-input.md), [reference examples](validation-reference.md) |
| Execution and packaging | [Fuzzing](validation-fuzzing.md), [resources](validation-resources.md), [platforms](validation-platforms.md) |

[Back to the project](../README.md) · [MIT License](../LICENSE)
