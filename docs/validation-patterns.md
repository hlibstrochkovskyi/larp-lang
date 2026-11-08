# Enums, matching, and propagation validation

Date: 2026-11-09. Scope: the `0.3.0-dev` implementation of
[RFC 0003](rfcs/0003-enums-patterns-and-propagation.md). The public release remains
`v0.2.0-preview.1`. This record does not publish or merge a release.

## Executed local checks

The real compiler lowers nominal enums, exhaustive matches and `?` through typed
CFG IR and independently verified bytecode into the VM. Source translation and
host evaluation are not used. Initial local checks passed:

| Check | Observed result |
| --- | --- |
| Formatting and workspace Clippy with warnings denied | Passed |
| Locked offline workspace tests including final loader regression | 552 passed |
| Optimized compiler tests | 209 passed |
| Corpus inventory | 100 active, none pending; inventory alone executes nothing |
| Real conformance | 100/100 in ordinary, stress, artifact/stress and formatted artifact/stress modes |
| Existing reference | 8 samples, 40 VM executions, 113 CLI commands |
| Existing README example | 1 sample, 5 VM executions, 15 CLI commands |
| New pattern guide | 2 samples, 10 VM executions, 29 CLI commands |
| Multi-file project fixtures | 2 projects, 10 VM executions, 42 CLI commands |
| Manifest project workflow | 2 workflows, 14 VM executions, 29 CLI commands |
| Packaging-gate unit tests | 28 passed; synthetic reports do not prove platform compatibility |

The new source cases test managed payload identity, closure captures, enum arrays,
propagation evaluation order, local suppression cleanup, deferred result errors,
nominal mismatches, incomplete/repeated patterns, recursive enums and all-variant
snapshot rejection. Compiler integration tests execute source and independently
loaded artifacts under ordinary/stress collection. They also cover constructor
initialization, module privacy, first-class constructors, result success-type
changes, and regional helper errors that do not cancel a selected commit.

Independent review found that unused enum entries in the artifact type table
could evade canonical identity checks after module reconstruction. The loader now
checks every decoded enum descriptor, including entries referenced only by unused
containers/functions. A regression appends noncanonical descriptors with both an
existing and an unknown ID to a valid artifact and requires rejection. All 25
artifact tests passed after this correction. A separate lambda regression keeps
return inference correct when every match arm returns from a block body.

## Remaining acceptance gates

A clean tracked archive of implementation commit
`33ea7fb7861de9f17772dc89f072aadb22ea1b1c` produced the real version-3
[frozen fixture](../tests/compatibility/artifacts/README.md#version-3-production-and-verification).
The producer and current CLI executed its exact bytes after the source path was
made unavailable, with ordinary/stress GC and stdout `49\ncap\n`. The version
gate passed: writer 3; frozen historical readers 1, 2 and 3; JSON schemas 1.
The existing version-1/2 source, artifact bytes and manifest records are unchanged.
Resource smoke cases also completed with their expected ordinary/stress outcomes.

Exact-commit CI, two clean offline release builds, extracted-candidate execution
and the Ubuntu 24.04/26.04 same-archive matrix remain required before full M9
acceptance. The packaged CLI now executes the practical pattern guide as well as
the complete corpus, multi-file fixtures and manifest workflow. Candidate reports
bind the guide/contracts/driver and binary hashes and require complete matching
sample inventories and execution counts across both hosts.

Raw local evidence is retained in ignored `artifacts/m9*` directories. Finite
programs, review and tests do not prove general language correctness, cross-machine
reproducibility, additional platforms, a native ABI or scenario application
invariants. No tag, merge or release is authorized by this validation record.
