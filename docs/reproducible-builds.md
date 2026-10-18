# Local reproducible release-build check

Status: an executed local Linux x86-64 baseline. This checks the CLI binary,
not release publication, supported platforms, or a cross-machine/toolchain
reproducibility guarantee. The maintainer has selected MIT licensing; the
historical baseline commit below predates that license addition.

## Run the check

```sh
python3 tools/check_reproducible_build.py
```

The default creates a unique directory in ignored
`artifacts/reproducible-builds/`. `--output artifacts/reproducible-builds/my-check`
selects a new or empty directory; nonempty directories are rejected. Existing
outputs, generated sources, build trees, caches, and logs are retained. There is
no cleanup or publication step. `--jobs N` controls compilation concurrency.

`--revision COMMIT` selects a tracked commit, defaulting to `HEAD`. In the managed
workspace, the script resolves the authoritative Git directory through
`tools/git-local.sh`, which selects `.git-foundation`. It never substitutes the
unborn ordinary `.git` history. In a normal clone without `.git-foundation`, it
uses that clone's Git metadata. `--git-dir PATH` explicitly selects metadata.
All Git operations are read-only: resolve the commit, read its timestamp/listing,
and create an archive. The working-tree contents and uncommitted changes are not
build inputs. Commits containing private/generated root paths are rejected;
symbolic links and other special archive members are rejected rather than
followed.

One `git archive` supplies both builds. The driver extracts it into two distinct
clean source paths, deliberately containing spaces, and gives each a distinct
initially absent Cargo target directory. Both build the real CLI with:

```text
cargo build -p larp-cli --bin larp --release --locked --offline
    --target x86_64-unknown-linux-gnu --jobs 2
```

The current production workspace needs no registry dependencies. A fresh local
Cargo home avoids global Cargo configuration/cache changes; compiled artifacts
are never shared between the two target directories. Build wrappers are disabled,
incremental compilation is disabled, Cargo cache auto-cleaning is disabled, and
potentially conflicting inherited build/profile/target flags are removed. Locale
and timezone are fixed; `SOURCE_DATE_EPOCH` uses the selected commit's timestamp.
The existing compiler, linker environment, system libraries, and toolchain
installation remain unchanged. No package manager, downloads, global settings,
Git index writes, or repository modifications are needed for the builds.

For each build the driver remaps its source and target prefixes to stable
`/larp/source` and `/larp/target` names. It passes each complete compiler argument
through `CARGO_ENCODED_RUSTFLAGS`, separated with ASCII unit separator, so spaces
in real workspace paths do not split an argument. This encoding follows the
[Cargo environment-variable contract](https://doc.rust-lang.org/cargo/reference/environment-variables.html).
Rust's [path-remapping option](https://doc.rust-lang.org/rustc/command-line-arguments.html#--remap-path-prefix-remap-source-names-in-output)
changes source-name prefixes in compiler output; it is not a general replacement
for pinning the linker, system libraries, build scripts, or compiler version.

## Evidence and failure behavior

`report.json` records the exact commit/archive hash, Git metadata location,
driver hash, compiler/Cargo versions and executable paths, target/platform,
controlled environment, names of removed environment keys, complete per-build
commands/remapping arguments, elapsed time, binary sizes and SHA-256 values.
It also records byte-by-byte equality and the CLI smoke results. Credentials and
the complete inherited environment are not dumped.

Both binaries must match by SHA-256 and by a direct byte comparison. Each must
then pass source check/run, artifact build/check/run, using a collection pipeline
with GC stress and exact output `5\n50\n`. Both independently produced bytecode
artifacts must match as well. These checks execute the actual compiler, loader,
verifier, and VM; a binary hash alone does not establish that they work.

Build failures, mismatches, and smoke failures return nonzero and leave evidence
marked `failed`. Build logs, both source/target trees, binaries, source archive,
smoke source, and artifacts stay available for investigation. The script does
not repair a mismatch by copying one binary over the other. Builds have a
ten-minute timeout each; a timed-out build's process group is terminated so
compiler/linker descendants do not continue running. Smoke commands have fifteen
seconds each. A dedicated hosted CI job runs the check and retains report/build
logs as `reproducibility-evidence`; complete build trees remain local evidence.

## Executed baseline

On 2026-10-19 local time, two clean release builds of commit
`5d9a1c23fef20b7c9c27d14b4d45eb4688c66797` completed successfully on x86-64 Linux
7.2.9/glibc 2.44, using Arch Linux Rust/Cargo 1.99.0 and LLVM 23.1.1.
The retained first report is
`artifacts/reproducible-builds/20261018T232702Z-ee9874c2/report.json` (UTC path).

Both CLI binaries were exactly 1,794,904 bytes and matched byte-for-byte:

```text
SHA-256 b0a99798927c62d924c3ea4f274dd776d1d808342ff7a626d3f609fa1cb6e9ea
```

The shared tracked-source archive SHA-256 was
`1f3d1e5f4f46a152e9445fb99926382544350e0ef13986dd7e027ef2539d30d8`.
All ten CLI smoke commands succeeded, including four real source/artifact VM
executions with exact stdout and empty stderr. The two bytecode artifacts also
matched byte-for-byte. Sources and all outputs were preserved.

A second invocation with explicit `--git-dir .git-foundation` and the same
revision repeated both clean builds and all ten smoke commands successfully.
Its report is `artifacts/reproducible-builds/20261018T232822Z-dcea7ace/report.json`.
The binary hash was unchanged; the final driver also found none of the two
builds' exact source/target path prefixes embedded in either binary. Reusing the
first nonempty output directory was separately rejected with exit status 2.

After MIT licensing metadata was committed, the reviewed driver also checked
`66d9877200ca7044f024b29fc014c6a5d1463b6d` in
`artifacts/reproducible-builds/mit-baseline/report.json`. Both release builds
again produced the same 1,794,904-byte binary and SHA-256 above, no exact original
source/target prefixes were found, and all ten CLI smoke commands passed. The
report's driver hash matches the reviewed script. This does not mean future
compiler changes must retain this hash; each measured pair must match internally.

These observations demonstrate equality for the measured commits, one existing
toolchain/system, and one controlled local build method. They do not establish equality across
different Rust versions, operating systems, linkers, system libraries, arbitrary
environment changes, future dependencies, checkout timestamps, or all archive
formats. Only the `larp` executable is compared, not every intermediate object,
Cargo metadata file, source archive path, or external debug artifact. Artifact
source maps intentionally contain user source names/text and are not a promise
of path-independent output for arbitrarily named programs.

A release/distribution plan still needs explicit platform decisions, toolchain
provisioning, packaging and provenance policies, and repeated hosted
verification. This check is local evidence for that work, not its completion.
