# Manifest project workflow validation

Date: 2026-10-23. Scope: the `0.3.0-dev` implementation of
[RFC 0002](rfcs/0002-project-workflow.md). The current public release remains
`v0.2.0-preview.1`; these checks do not publish or merge a release.

## Executed local checks

The development CLI executed the scaffold and manifest workflow through the
real compiler, verifier and VM. No source translation or host evaluation was used.

| Check | Observed result |
| --- | --- |
| Workspace formatting and Clippy with warnings denied | Passed |
| Locked offline workspace tests | 504 passed |
| Optimized compiler tests | 193 passed |
| Corpus inventory | 89 active, none pending; inventory alone does not execute programs |
| Real conformance execution | 89/89 in each of ordinary, stress GC, artifact/stress GC, and formatted artifact/stress GC modes |
| Practical reference | 8 samples, 40 VM executions, 113 CLI commands |
| README example | 1 sample, 5 VM executions, 15 CLI commands |
| Existing multi-file pitch fixture | 1 project, 5 VM executions, 23 CLI commands |
| New project workflow driver | 2 workflows, 14 VM executions, 29 CLI commands |
| Release-tool unit tests | 26 passed; synthetic reports test gate decisions, not platform compatibility |
| Version and frozen artifact checks | `0.3.0-dev`; artifact writer 2, historical readers 1 and 2; diagnostic schema 1 |
| Resource measurement smoke run | Existing ordinary/stress cases completed with their expected outcomes |

Commands and repeatable entry points are in [CONTRIBUTING](../CONTRIBUTING.md),
[project usage](projects.md), and `tools/check_project_workflow.py`. Raw local
reports are retained under ignored `artifacts/`; they are not public source.
A later source change requires its own checks.

The workflow driver uses an existing executable. It creates a project with
`larp new`, checks and runs it, discovers its manifest from a nested directory,
selects it explicitly from elsewhere, and builds both default and explicit
outputs. It also executes a fixture with sibling-directory imports, stdin and
Unicode arguments. Original source and manifest paths are moved out of the way
before each independently loaded artifact runs, including stress GC. It records
binary/driver/source hashes and every command's cwd, input, output and status.

The 19 CLI workflow integration tests additionally cover malformed/unknown/
duplicate manifest fields, real TOML forms, source spans, portable names,
manifest size/encoding/type checks, nearest-invalid discovery, canonical
symlink containment, argument/fuel forwarding, source/manifest/output
preservation, and explicit-file compatibility.

## Review findings corrected before acceptance

- A legacy-grammar entry initially bypassed the aggregate source-line budget.
  Project entry bytes and lines are now checked before selecting a grammar;
  `check`, `run` and `build` consistently reject the oversized entry with
  `LARP-M005`. The regression also checks that no default output directory is
  created on failure.
- Opening a manifest before checking its file type could block on a FIFO.
  The CLI now checks canonical-path metadata first and checks the opened file
  again. An ordinary-filesystem FIFO reproduction returned `LARP-J001`
  immediately under a three-second subprocess deadline. This is not a hostile
  concurrent-filesystem guarantee.
- Dash-prefixed explicit source and output filenames remain usable. Paths that
  collide with recognized options can be disambiguated with a `./` prefix.
- Candidate packaging initially reserved `THIRD_PARTY.md` as generated output.
  It now preserves the committed dependency inventory and upstream vendor
  licenses, and places Rust runtime documentation at `THIRD_PARTY/RUST.md`.

## Remaining publication boundaries

CI executes the same workflow against the actual extracted candidate binary on
Ubuntu 24.04 and 26.04. The matrix requires identical archive/binary identity,
complete successful workflow commands, matching source/driver hashes, and
execution after source paths become unavailable. Hosted results must be checked
for the exact PR revision; this local record alone does not establish them.

The candidate also requires two clean offline release builds to produce equal
binary bytes and an unchanged checksummed payload after execution. Vendored
sources retain their original notices; [THIRD_PARTY.md](../THIRD_PARTY.md)
records provenance. These finite checks do not establish general language
correctness, hostile-filesystem isolation, cross-toolchain reproducibility,
additional platforms, a package registry, or a new public release.
