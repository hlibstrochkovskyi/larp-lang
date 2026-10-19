# Candidate compatibility gate validation

Date: 2026-10-19. The gate is implemented, and the Ubuntu matrix passed for
`c6083b05f329b910da302cfb77eaf3d1c5fd0c98`; see the hosted results below. The local
evidence tests the driver and extracted compiler/VM on Arch Linux and is recorded
separately from Ubuntu support evidence.
See the [platform policy](platforms.md) and [distribution contract](distribution.md).

## Executed local candidate

The tracked source was commit `b381fd3178ab78115317a4e37796e72b5401ed1e`.
An isolated official Rust 1.99.0 installation was extracted from the upstream
distribution after checking its published SHA-256:
`891c6366d7100feda0bca4c03ce63f3c9ac827cbebbc283e7433061d42c6a376`.
It changed no global compiler installation. Two independent clean builds passed
the existing reproducibility driver and its source/artifact smoke contracts.
Both CLI binaries had SHA-256
`a9d7ad78e4abcfe4cbb1027db7252fa4d898974814979805645fb5e5ba06d37f`.

Packaging retained that compiler's complete runtime notice inventory and wrote
two byte-identical 1,103,463-byte archives. The candidate archive SHA-256 was
`df736dbcda834c3f40e9d2af5d48990cf056fe0e1bb81c8a35aecbe758a74af7`.
The new compatibility driver executed this archive using its archived harness
and CLI, without rebuilding:

| Path | Result |
| --- | --- |
| Archived corpus inventory | 89 active, zero pending |
| Source, ordinary GC | 89/89 |
| Source, GC stress | 89/89 |
| Source and loaded artifacts, GC stress | 89/89 |
| Formatted source and loaded artifacts, GC stress | 89/89 |
| Payload inventory before and after execution | All 274 checksummed files matched |

These modes include the existing exact text/JSON diagnostic and runtime-span
contracts. The driver recorded nine subprocess outcomes, executable mode 0755,
ELF x86-64 identity, actual toolchain provenance, and the host: Arch Linux,
kernel `7.2.9-arch1-1`, glibc 2.44. ELF inspection reported dependencies
`libgcc_s.so.1`, `libc.so.6`, and `ld-linux-x86-64.so.2`; its highest observed
GLIBC version reference was `GLIBC_2.34`. Those references are not a measured
minimum operating-system compatibility claim.

Retained ignored evidence:

- `artifacts/reproducible-builds/platform-official/report.json`
- `artifacts/package-platform-official/package-report.json` and both archives
- `artifacts/compatibility-platform-local/compatibility-report.json` and extraction

The candidate's source commit predates the new checker and platform documents;
the checker ran from the working tree, while the conformance scripts and fixtures
came from the checksummed archive. Hosted CI must package and test the actual
new commit independently.

## Rejection and foundation checks

Two real candidate-driver negative controls passed: an incorrect archive sidecar
and an incorrect expected source revision were rejected with `status: failed`
and zero execution commands. Reusing either nonempty output directory exited
with usage status 2 and preserved the prior report byte-for-byte. Outputs are
retained in `artifacts/candidate-negative-controls/`.

Seven Python regression tests passed, including mismatched archive/binary/checker
identities, missing/wrong OS, old revision, failed/incomplete conformance evidence,
changed/omitted payload files, and parent paths in checksum entries. Synthetic
reports in these unit tests exercise gate decisions; they are not platform or
LARP execution evidence. A separate read-only reviewer reran all seven tests and
checked the workflow's producer/download/collector dependencies.

Required foundation checks passed: Rust formatting, Clippy with warnings denied,
388 workspace tests, 138 release compiler tests, the 89-case corpus inventory,
ordinary conformance, and combined formatting/artifact/GC-stress conformance.
Workflow YAML parsed successfully and the source inspection confirmed that both
compatibility jobs download one producer artifact without invoking Cargo.

The hosted gate pins the build/check/fuzz preparation image to Ubuntu 24.04,
executes one packaged binary on Ubuntu 24.04 and 26.04, and compares both reports
in a separate aggregate job. Existing bounded fuzz parameters and oracles are
unchanged. Missing or failing host jobs leave the matrix unverified.

## Hosted exact-commit matrix

The [Compiler and conformance run](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39233527314)
passed for `c6083b05f329b910da302cfb77eaf3d1c5fd0c98`. Its five jobs all concluded
`success`: compiler/check, reproducible-build, both compatibility hosts, and the
cross-report aggregate. The same archive was produced on Ubuntu 24.04 and
downloaded/executed on both hosts without rebuilding its CLI.

```text
larp-0.1.0-dev-x86_64-unknown-linux-gnu-c6083b05f329.tar.gz
SHA-256 1d89e1601e0d8d1edcd85f77b645ef3cb7b5e130c4f59a9bfc389d8166d9311b
```

| Execution host | Actual image version | Actual libc | Packaged corpus |
| --- | --- | --- | --- |
| Ubuntu 24.04.5 LTS, x86-64 | `20261012.334.1` | glibc 2.39 | 89/89 in each of four modes |
| Ubuntu 26.04.1 LTS, x86-64 | `20261012.163.1` | glibc 2.43 | 89/89 in each of four modes |

The [Ubuntu 24.04 job](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39233527314/job/117997181556)
and [Ubuntu 26.04 job](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39233527314/job/117997181476)
each reported the same archive digest and passed all required execution checks.
The [aggregate job](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39233527314/job/117997290774)
downloaded both reports, checked their full revision/platform/corpus identities,
and confirmed identical candidate bytes.

Retained workflow artifacts include the [candidate archive](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39233527314/artifacts/12177972889),
[Ubuntu 24.04 report](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39233527314/artifacts/12178257804),
[Ubuntu 26.04 report](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39233527314/artifacts/12177257734),
and [aggregate report](https://github.com/hlibstrochkovskyi/larp-lang/actions/runs/39233527314/artifacts/12178496585).
They expire on 2026-11-02 under the workflow's 14-day retention policy. Artifact
ZIP digests identify their GitHub upload containers, not the tar.gz candidate
digest above. Local download of these ZIP files was unavailable because the file
host did not resolve in the terminal environment; job conclusions, runner-image
versions, matching candidate hashes, and aggregate success were inspected through
the connected GitHub API and decoded job logs. The reports remain in Actions.

This proves the recorded candidate's measured behavior on these exact hosted
images, with the limits stated in the platform policy. Later source or packaging
commits require their own matrix; this is neither a versioned Release nor a
cross-host reproducible-build claim.
