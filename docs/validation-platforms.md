# Candidate compatibility gate validation

Date: 2026-10-19. The gate is implemented; the intended Ubuntu matrix requires
its own exact-commit hosted outcomes. The local evidence below tests the driver
and the extracted compiler/VM on Arch Linux. It does not establish Ubuntu support.
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
