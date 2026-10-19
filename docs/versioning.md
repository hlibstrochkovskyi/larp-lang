# Versioning and compatibility

Status: the workspace version is `0.1.0-dev`. The language remains experimental,
and no named preview tag or GitHub Release has been published. This policy
defines version identities and publication criteria; it does not declare the
[preview readiness gate](preview-readiness.md) closed or implement publication.

## Independent version identities

| Identity | Current value | Meaning |
| --- | --- | --- |
| Toolchain version | `0.1.0-dev` | CLI and workspace package version from the root Cargo manifest |
| Language specification | `0.1` | The [proposed language specification](spec/v0.1.md), with experimental implementation status |
| Artifact wire version | Writer `2`; reader `1`, `2` | Explicit encodings accepted by the artifact decoder, subject to verification |
| Diagnostic JSON schema | `1` | The [structured diagnostic protocol](diagnostics.md) |
| Version-info JSON schema | `1` | The machine-readable CLI identity described below |

These values evolve independently. A toolchain patch does not imply a new
language specification or artifact encoding. A language specification number
does not assert that every proposed feature is implemented. Consult
[implementation status](implementation-status.md) for the supported subset.
Artifact writer/reader values come from the actual encoder/decoder constants;
the diagnostic schema comes from the renderers. A version string is not a
substitute for inspecting those contracts.

## Toolchain versions and preview series

LARP uses [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html)
notation. SemVer identifies major-zero development and prereleases as unstable;
the more specific pre-1.0 rules here define how LARP communicates its changes.
There is no stable language or public Rust library API promise yet.

- Development builds use `0.y.z-dev`, currently `0.1.0-dev`. Multiple development
  commits can share that version; retain the full commit and archive digest.
- The first named preview series uses `0.y.z-preview.N`, beginning with a
  positive decimal `N` without leading zeroes. For example,
  `0.1.0-preview.1` would be a preview identity, not an existing release.
- Numbered previews in a series may change the experimental design. Every
  intentional source, behavior, CLI, or library incompatibility must be explicit
  in its release notes, with the affected construct and migration or rebuild
  action. An increasing preview number alone does not promise compatibility.
- Between preview series, increment the minor version and reset the patch when
  introducing incompatible supported source or CLI behavior. Use a patch
  increment for compatible fixes. Acceptance of invalid programs is not a
  supported behavior; describe corrections that reject such programs in the
  release notes as well.
- A version without a prerelease suffix identifies the `release` channel. That
  channel name alone does not imply language stability: `0.y.z` remains pre-1.0.

There is no source edition switch. Changing a version cannot select historical
typing or execution semantics inside the current compiler. Use a retained older
toolchain to reproduce an earlier version's behavior, or migrate the source.

## Machine-readable identity

Human-readable identity remains available as `larp --version`, printing
`larp VERSION (objects, scenarios, and memoized lazy values)` followed by a
newline. The descriptive suffix may change. For tools, use:

```sh
larp --version --json
```

The command emits one JSON object followed by a newline on stdout, exits
successfully, and leaves stderr empty. For the current workspace it reports:

```json
{"schema_version":1,"name":"larp","version":"0.1.0-dev","channel":"development","language_spec":"0.1","language_status":"experimental","artifact":{"write":2,"read":[1,2]},"diagnostics_schema":1}
```

`schema_version` versions this version-info object separately from diagnostics.
`version` is the actual toolchain package version. `channel` is `development`
for the `dev` prerelease or a `dev.*` prefix, `preview` for a `preview.*` prefix,
`release` when there is no prerelease, and `prerelease` for other prerelease
labels. This classification does not validate eligibility for publication: the
named-preview policy above still requires a positive numeric `N`. `language_status`
describes the language's experimental status, regardless of the channel.
`artifact.read` lists explicit accepted wire versions; `artifact.write` is the
encoding produced by this CLI. `diagnostics_schema` identifies its diagnostic
JSON protocol. None of these fields identifies the source commit or executable
bytes: use the package's `BUILD.json`, SHA-256 sidecar, and payload checksums.
The package records this object as `BUILD.json.toolchain_identity`. Candidate
compatibility checks compare it with the real extracted executable's JSON
identity and run the archived version-contract driver.

Consumers must validate the schema they support before interpreting fields and
reject an unknown schema explicitly. Within a schema, tolerate additional
fields. Removing or renaming required fields, changing their types, or changing
their meaning incompatibly requires a new schema. Diagnostic and version-info
schema numbers are incremented separately. Human diagnostic prose, formatting,
and help text are not a machine protocol; use structured diagnostic fields.

## Bytecode and updates

Artifacts carry their own explicit wire version. Loading always includes the
mandatory verifier, including when the wire version is recognized. Validation
fixes may reject previously accepted invalid modules without changing their
encoding version. Recognition of an encoding never guarantees execution of an
invalid module or bypasses effect, type, region, or resource checks.

No long-term artifact compatibility beyond an executable's advertised read list
is promised. A future toolchain can remove an old read version with an explicit
compatibility note. An unsupported artifact version must be rejected; rebuild
from retained source with the intended toolchain. Artifact versions do not
provide a downgrade path or imply stable internal Rust crate APIs.

The version-contract driver checks newly emitted bytecode's actual wire header
and loads genuine frozen v1 and v2 artifacts from
[`tests/compatibility/artifacts/`](../tests/compatibility/artifacts/). The v1
fixture was produced by the historical encoder at commit `044b02d`; the v2
fixture was produced at `b381fd3`. Neither fixture is manufactured by changing a
current artifact's version header. Executing these fixtures supplies evidence
for sampled historical decoding, verification, and VM behavior; it does not
establish acceptance of every module produced by a historical compiler.

Updating means selecting a specific newer toolchain, reading its change notes,
checking the intended [platform evidence](platforms.md), and rebuilding/testing
the application's source. There is no updater, Cargo package publication, or
automatic artifact migration in the current toolchain. Retain source and the
previous executable when comparing behavior.

## Publication and evidence

Published versioned bytes are immutable. Never replace a published tag or
overwrite a released archive under the same identity. Corrections require a
new version, even if the change is only packaging or documentation. Version
precedence does not authenticate an executable; exact source revisions, archive
and payload hashes, and actual build provenance remain separate evidence.

Before publishing a named preview, retain evidence for its exact source commit:

1. Required formatting, lint, workspace and release tests, corpus inventory,
   all four conformance modes, and executable reference checks pass.
2. Independent native release builds and packaged archives pass their
   reproducibility checks. The identical candidate archive passes the
   [Ubuntu 24.04/26.04 execution matrix](platforms.md), with matching full
   revision, archive digest, executable digest, and observed version identity.
3. The source and candidate include the maintainer-selected MIT license and
   runtime notices from their actual Rust distribution. Package checksums and
   provenance pass the [distribution checks](distribution.md).
4. Release notes identify the implemented/deferred language subset, supported
   platforms, experimental scenario restrictions, known limits, and changes
   requiring migration or rebuilding. Open design-review findings remain
   visible; passing finite tests does not establish whole-language correctness.
5. The version-contract check exercises the real CLI's plain/JSON identities,
   diagnostic schema, newly emitted artifact header, and genuine historical
   artifact load/run behavior. Its retained report must agree with the
   candidate's version and declared formats. Each compatibility host runs this
   archived driver against the same extracted executable.

The version-contract driver is invoked as:

```sh
python3 tools/check_version.py --bin target/debug/larp --output artifacts/version-contract
```

Use a new or empty output directory and the intended executable. The driver
checks the current implementation; it does not create a release or tag. The
candidate workflow continues to upload review archives. A publication command,
tag-triggered release workflow, signatures/attestations, and a support lifetime
remain outside the current implementation. No preview publication or maintainer
design acceptance is implied by this policy.

## Executed local validation

On 2026-10-19, the current CLI passed 13 version-contract commands, including
five real VM executions: one newly emitted artifact and both frozen historical
artifacts with ordinary/stress GC. Identity reported toolchain `0.1.0-dev`,
experimental specification `0.1`, writer 2, readers 1/2, and diagnostic schema 1.
Evidence, driver/validator fingerprints, generated artifacts, and exact outputs
remain in ignored `artifacts/version-contract-validated/`.

A separate copied fixture inventory intentionally expected incorrect stdout
from the historical v1 module. The real VM printed `5\n50\n`; the driver rejected
that mismatch and saved a failed report. Reusing its nonempty output directory
exited with status 2 and preserved the report byte-for-byte. The original frozen
fixtures were unchanged; this control remains in
`artifacts/version-negative-controls/`.

Required Rust formatting and Clippy checks passed, along with 390 workspace tests,
138 release compiler tests, ten Python gate/identity tests, the 89-case inventory,
ordinary conformance, and formatted/artifact/GC-stress conformance. These local
results test the implementation; exact-commit hosted packaging and platform
results must still be inspected independently for each candidate.
