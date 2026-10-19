# Preview platform policy

Status: **intended baseline; exact-commit compatibility CI has not yet run**.
This policy defines the first preview's platform gate. Existing local builds,
earlier hosted checks, and candidate smoke tests do not establish the matrix
below. Record successful workflow runs, full source revisions, archive digests,
and runner image versions before describing a candidate as validated here.
The [local gate validation](validation-platforms.md) records executed checks and
their limits separately from hosted compatibility evidence.

## Intended build and execution matrix

| Work | Runner label | Toolchain or executable |
| --- | --- | --- |
| Compiler formatting, linting, workspace/release tests and source conformance | `ubuntu-24.04`, x86-64 | Official Rust 1.99.0 |
| Independent release builds and candidate packaging | `ubuntu-24.04`, x86-64 | Official Rust 1.99.0; target `x86_64-unknown-linux-gnu` |
| Existing bounded fuzz workflow | `ubuntu-24.04`, x86-64 | Rust 1.99.0 for stable preparation; separately pinned nightly instrumentation described in [fuzz validation](validation-fuzzing.md) |
| Downloaded candidate compatibility | `ubuntu-24.04`, x86-64 | The archive produced by the release-build job |
| Downloaded candidate compatibility | `ubuntu-26.04`, x86-64 | The identical archive produced by the release-build job |

The release-build job produces one candidate for both compatibility jobs. Each
job downloads and verifies that archive, then executes its extracted `bin/larp`.
It does not rebuild Rust source, replace the executable with a runner-local
build, or generate a separate candidate for its OS. Rust may already exist on a
hosted image; its presence is not a runtime dependency of the packaged CLI.
Instrumented fuzzing retains its existing separate nightly toolchain rather than
claiming that its sanitizer build runs on stable Rust.

The [official runner image inventory](https://github.com/actions/runner-images/blob/main/README.md)
lists both selected x64 labels. Explicit OS labels avoid the OS migration of
`ubuntu-latest`, but they do not pin an image hash: installed software and images
receive weekly updates. Record the actual image version in each run's evidence.
Ubuntu 22.04 is outside this intended matrix; its runner retirement is tracked
in the [official deprecation announcement](https://github.com/actions/runner-images/issues/14254).
This choice does not establish whether a particular candidate can run there.

## Candidate acceptance criteria

All checks below must pass for the same full source commit and the same candidate
archive on both compatibility runners. A missing, skipped, cancelled, or failed
job leaves the matrix unverified for that commit.

1. The compiler/check job passes its required formatting, lint, Rust tests, and
   source conformance gates. The independent-build job passes its two-build
   byte-equality check and packages the resulting CLI with the selected MIT
   license and the actual Rust distribution's runtime notices.
2. Each compatibility job verifies the archive's SHA-256 sidecar and records the
   archive digest. Those digests must identify the same downloaded bytes. It
   verifies the extracted payload against `SHA256SUMS`, including `bin/larp`,
   `BUILD.json`, archived fixtures, and harness scripts.
3. `BUILD.json` identifies the exact expected source commit,
   `x86_64-unknown-linux-gnu` target, and Rust 1.99.0 build toolchain. The actual
   binary digest and version agree with that provenance. A matching version
   string alone is insufficient to identify a build.
4. The extracted binary executes the full active conformance inventory shipped
   in that archive, using its archived harness and fixtures. Test the ordinary
   source path, source with GC stress, loaded artifacts with GC stress, and
   formatted source plus loaded artifacts with GC stress. These runs include
   the existing text/JSON contracts: exit status, exact stdout and inspection,
   primary error identity, and exact runtime byte spans and scalar coordinates.
   No pending case may be silently counted as executed.
5. Each job retains its environment/provenance report and command outcomes,
   including runner OS/image, CPU architecture, actual libc information, source
   revision, archive and executable hashes, and conformance results. The final
   workflow outcome must expose either compatibility job's failure.

The archived conformance commands, run from the extracted package root, are:

```sh
python3 tools/check_corpus.py
python3 tools/run_conformance.py --bin ./bin/larp
python3 tools/run_conformance.py --bin ./bin/larp --gc-stress
python3 tools/run_conformance.py --bin ./bin/larp --artifacts --gc-stress
python3 tools/run_conformance.py --bin ./bin/larp --formatted --artifacts --gc-stress
```

Python runs the test harness; LARP source and bytecode execute through the
packaged compiler, verifier, and VM. Checksum validation detects changed content.
It does not by itself authenticate a publisher or provide a signature or
attestation. See [distribution](distribution.md) and
[reproducible builds](reproducible-builds.md) for those separate contracts.

## Evidence limits and other platforms

A passing matrix establishes the measured candidate's behavior on the recorded
GitHub-hosted images. It does not establish compatibility with every installation
of either Ubuntu release, an arbitrary GNU/Linux distribution, a minimum kernel
or GLIBC release, older CPUs, or every environment change on a later runner image.
Observed ELF/GLIBC version references describe binary requirements; they do not
replace executing the archive on a claimed platform. The same-binary execution
gate also does not prove reproducible compilation across different hosts.

Windows, macOS, BSD, WSL, other Linux distributions, musl targets, ARM/AArch64,
and other CPU architectures have no validated preview support under this policy.
The Rust source may be portable to some of them; a successful source build or a
report from another environment is useful evidence, but it does not silently
extend the candidate matrix. Add an explicit target, packaging path, and retained
execution evidence before making an additional platform claim.

Later candidate commits require their own matrix results. Changing an OS label,
target, Rust version, runtime dependency, or packaging method requires reviewing
this policy and rerunning the affected gates. No minimum support lifetime or
versioned Release is established by this draft.
