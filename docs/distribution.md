# Distribution and candidate packaging

The current public prerelease is
[`v0.2.0-preview.1 — File Modules`](https://github.com/hlibstrochkovskyi/larp-lang/releases/tag/v0.2.0-preview.1).
It includes a Linux x86-64 GNU archive, its SHA-256 sidecar, and release evidence.
See the [publication record](releases/0.2.0-preview.1-publication.md) for exact
filenames, digests, source revision, and compatibility results. LARP remains an
experimental developer preview under the [MIT License](../LICENSE). The
[first preview](releases/0.1.0-preview.1-publication.md) retains its original bytes.

The candidate packager described below produces reviewable archives and
exercises their extracted CLI. Running it does not publish a tag or GitHub
Release; publication is a separate maintainer-authorized action.

## Build a candidate

Use Python 3.11+, an official Rust toolchain with its runtime notice files, and
native x86-64 GNU/Linux:

```sh
python3 tools/check_reproducible_build.py --output artifacts/reproducible-builds/package-input
python3 tools/package_candidate.py \
    --report artifacts/reproducible-builds/package-input/report.json \
    --output artifacts/package-candidate
```

Both directories must be new or empty. Outputs, source snapshots, builds,
archives, and extractions are retained; repeat with different directory names.
The first command selects one tracked commit, builds two independent release
CLIs offline, and checks both. See [build reproducibility](reproducible-builds.md)
for its exact scope, toolchain controls, and Git metadata selection.

Packaging accepts only a complete passed native Linux report. It independently
rechecks the source-archive digest, both binary digests, and their actual byte
equality. Source files come exclusively from that verified Git archive, rather
than the current working tree. The archived Cargo metadata and LICENSE must
identify MIT. Private/generated roots, nonregular files, traversal paths,
duplicate entries, and reserved package filenames are rejected. These are
consistency checks for our own build reports, not a service for accepting
untrusted executables or third-party uploads.

The compiler must still report the exact version recorded at build time.
Its `share/doc/rust/COPYRIGHT-library.html` and `licenses/*.txt` are copied
unchanged into the package. Rust [documents these distribution notices](https://github.com/rust-lang/rust/blob/1.99.0/COPYRIGHT)
as the standard-library source/dependency inventory. Some system compiler packages
omit that complete distribution layout; they can still build LARP, but this
packaging command requires the upstream notice files rather than substituting a
different compiler's attribution. No network lookup or global installation is
performed by the packaging command.

## Contents and use

The archive name includes its version, target, and source commit prefix:

```text
larp-VERSION-x86_64-unknown-linux-gnu-COMMIT.tar.gz
```

Its single root directory contains:

- `bin/larp`, with executable mode 0755.
- The complete tracked source snapshot, including LICENSE, examples, and docs.
- `PACKAGE.md`, with commands for using this extracted candidate.
- `BUILD.json`, with full source commit, target/version, toolchain information,
  source/binary digests, and observed GLIBC version references when available.
  New candidates also retain the actual CLI's `toolchain_identity` JSON object,
  including its language baseline, artifact versions, and diagnostic schema.
  Named previews include `preview`, binding a proposed tag and release-document
  hashes to that identity; this field does not assert publication or approval.
- `SHA256SUMS`, covering every other regular file, including the provenance and
  executable. The checksum file excludes itself to avoid a circular digest.
- `THIRD_PARTY.md` and `THIRD_PARTY/rust/`, preserving upstream runtime attribution
  and license texts from the exact compiler distribution. These components keep
  their own terms; the upstream inventory can cover more code than this CLI uses.

After unpacking and entering the single root directory:

```sh
sha256sum -c SHA256SUMS
./bin/larp --version
./bin/larp run examples/hello.larp
./bin/larp run examples/collections.larp --gc-stress
```

The prebuilt CLI needs no Rust installation. The source snapshot has no Git
metadata; clone the hosted repository for development history. Running the
candidate from its extracted directory needs no system installation or shell
configuration changes.

This candidate targets dynamically linked GNU/Linux x86-64. Its GLIBC references
come from [readelf version sections](https://sourceware.org/binutils/docs/binutils/readelf.html),
which describe binary requirements rather than demonstrating compatibility on an
older distribution. The [preview platform policy](platforms.md) defines explicit
build/execution hosts and the evidence required for each candidate. A build-host
smoke test does not establish that matrix, a minimum distribution/kernel version,
or support for other CPU/OS combinations. Checksums detect content changes; no signing or attestation
claim is made.

## Determinism and execution checks

The packager independently writes two archives, using the two verified build
binaries, and requires complete archive byte equality. Entries have sorted names,
fixed numeric owner/group IDs and names, normalized file/directory modes, and
the source commit's timestamp. Gzip has an empty filename and zero timestamp.
Current wall-clock time, output directory, host username, raw build paths, and
the complete inherited environment are not provenance inputs. Equality is
checked within a run; it is not asserted across arbitrary Python/zlib versions,
toolchains, source commits, or systems.

The first archive is extracted into a new directory. All file hashes and the
CLI's executable mode are checked, and GNU `sha256sum -c` is exercised when
available. The extracted binary must report its expected version, run hello
world, check/run the collection sample, and compile/check/run a loaded artifact.
All executions receive explicit EOF and have fifteen-second timeouts. Normal
and artifact collection execution use GC stress. The smoke artifact is outside
the unpacked payload, preserving its original checksummed contents.

`package-report.json` retains the exact archive hash/size, source revision,
packager fingerprint, outcomes, and actual CLI output. A sidecar `.sha256` covers
the complete archive. Failure after packaging begins is recorded, with partial
outputs retained. Passing hashes alone never stand in for successful VM runs.

## Executed local baseline

On 2026-10-19, packaging from the passed build report for commit
`a803be5a871c12837f6c32fe0480c4df10d28371` produced two identical archives of
1,066,600 bytes, including the runtime notices:

```text
SHA-256 3b6966c20af9e3c868d79496236285ceae88b9fd4ff8b0cf5707c3983bf99973
```

The local packaging test used the already installed official
`nightly-2026-10-17` toolchain (`1.101.0-nightly`, commit `5d8fec066`), which provides
the complete upstream notice layout. CI retains its pinned official Rust 1.99.0;
the package provenance always records the actual compiler. The system Arch
compiler's earlier experimental archives remain preserved separately and are not
the final notice-bearing candidates.

All 260 payload checksums passed both the Python verification and GNU
`sha256sum -c`. The extracted binary retained mode 0755. Seven real CLI commands
passed, including three VM executions; hello world printed
`Fake it till you compile.\n`, and source/artifact collection runs each printed
`5\n50\n`. The driver fingerprint matches the script used for that run. Evidence
and both archives remain in ignored `artifacts/package-with-runtime-notices/`.

CI builds its own exact-commit candidate after the independent-build check.
Reports/logs are saved as `reproducibility-evidence`; a successful candidate and
its archive checksum are saved as `larp-candidate-linux-x86_64`. This artifact is
for review and testing; it is distinct from the
[published first preview](releases/0.1.0-preview.1-publication.md). Each future
Release needs its own publication decision and retained evidence. No long-term
compatibility promise follows from candidate packaging.

## Execute the same candidate on the preview hosts

CI builds and packages on `ubuntu-24.04`, then downloads the identical candidate
artifact in separate `ubuntu-24.04` and `ubuntu-26.04` jobs. Neither compatibility
job installs a Rust toolchain or rebuilds the CLI. Python 3.11+ runs the archived
harness; GNU `getconf`, `readelf`, and `sha256sum` record/check the host and payload.

```sh
python3 tools/check_candidate.py \
    --input artifacts/downloaded-candidate \
    --revision FULL_SOURCE_COMMIT \
    --output artifacts/candidate-compatibility
```

The input contains exactly one `larp-*.tar.gz` and its `.sha256` sidecar. The
checker bounds the archive/extracted payload to 128 MiB and 8192 entries, requires
one normalized package root and regular files/directories, and verifies complete
payload checksums before execution. It checks exact revision, version, MIT,
target, recorded Rust 1.99.0, ELF architecture/mode, and runtime notice inventory.
The actual `--version --json` response must match `BUILD.json.toolchain_identity`.
The archived version-contract driver checks the reported encoder against real
emitted bytes and executes frozen historical v1/v2 artifacts on that host. Its
outputs stay outside the checksummed payload; Python bytecode caching is disabled
for the archived driver so imports do not modify package contents. See
[versioning](versioning.md) and the [fixture provenance](../tests/compatibility/artifacts/README.md).
These checks apply to our own CI-produced executable; they are not an execution
sandbox or publisher authentication for arbitrary downloaded programs.

The packaged corpus inventory and all four conformance modes in the platform
policy must pass. Commands run with EOF, captured stdout/stderr, and process-group
timeouts; a timeout, failure, pending case, or unexpected stderr fails the job.
For current candidates, the archived [project driver](../tools/run_projects.py)
executes the [multi-file inventory](../tests/projects/manifest.json) with the same
prebuilt binary. Each project passes source/source-stress, artifact/artifact-stress,
and formatted-artifact-stress runs. Source snapshots are retained under renamed
paths before artifact execution. Exact stdout/inspection, format idempotence,
manifest/binary hashes, and all five executed modes are recorded. Both hosts
must agree on this evidence; a missing mode or project fails the aggregate gate.
This extends candidate policy after the first preview, whose archived driver
retains its original single-file checks.

Payload hashes are checked again after execution. Each new/empty output directory
retains `compatibility-report.json`, its environment/image and libc/ELF details,
hashes, command outcomes, and extraction. Existing evidence is never overwritten.

CI retains the two reports as `candidate-compatibility-ubuntu-24.04` and
`candidate-compatibility-ubuntu-26.04`. The final `candidate-matrix` job checks both
reported host identities, exact source revision, all four executed modes, and
identical archive/binary/toolchain/corpus identities. A failure in either host
job prevents that aggregate gate from passing. `matrix-report.json` retains input
report hashes and both recorded environments. Each artifact expires after 14 days;
successful exact-commit evidence must be recorded before making a platform claim.
Compatibility artifacts also retain each archived driver's `version-report.json`;
the ordinary check job saves its own report as `version-contract`. Current
project execution records are retained in `project-contract` and each compatibility
host artifact as `projects/project-report.json`.
