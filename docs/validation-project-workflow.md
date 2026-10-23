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

## Hosted implementation evidence

Implementation commit
[`bac24b951704ac087f28338e7db89b8e37a82206`](https://github.com/hlibstrochkovskyi/larp-lang/commit/bac24b951704ac087f28338e7db89b8e37a82206)
passed all five jobs in
[run 39915882362](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39915882362):
workspace/conformance checks, reproducible build and packaging, candidate execution
on Ubuntu 24.04, candidate execution on Ubuntu 26.04, and the same-archive matrix.

The downloaded matrix report records 4078 checksummed payload files, 89
conformance cases, historical artifact versions 1 and 2, the existing pitch
project's five VM executions, and the two new workflows' 14 VM executions on
each platform. Both hosts used these exact candidate bytes:

- Archive: `larp-0.3.0-dev-x86_64-unknown-linux-gnu-bac24b951704.tar.gz`.
- Archive SHA-256: `56425e302ffd092d84ac4fd57faa8ceaee5401b2bd4e8adc27f89408d03f7c88`.
- CLI SHA-256: `e015f4c3c2ec9e3164d71d42fc83815ad4444dde9c0e21556f9be902f5017323`.

The CI artifact ZIP and its archive were downloaded and their recorded digests
checked. The existing-candidate checker independently passed again on the local
Arch Linux/glibc 2.44 host, including the manifest workflow. That additional run
is local evidence, not an expansion of the declared Ubuntu support matrix.
Two local clean Rust 1.99.0 release builds also matched one another; equality is
within each controlled build run, not between system and hosted toolchains.

[PR #1](https://github.com/hlibstrochkovskyi/larp-lang/pull/1) carries the change
for maintainer review. Documentation commits after the implementation commit
require their own exact-revision CI; they do not rewrite the evidence above.
No tag, release publication or merge is established by this record.
