# Historical bytecode compatibility fixtures

These are frozen artifacts emitted by genuine earlier LARP compilers. They test
whether the current loader, verifier, and VM still accept the advertised wire
versions. They are independent of the current encoder: a current encode/decode
round trip cannot establish historical compatibility.

The `.larpbc.hex` files contain lowercase hexadecimal text, with 32 decoded bytes
per complete line. Decode the complete file with `bytes.fromhex` before loading
it. The corresponding source files are provenance records; compatibility tests
must execute the decoded artifacts without compiling those source files again.

`manifest.json` uses schema version 1. Paths are relative to this directory.
`source_sha256` hashes the exact source bytes; `artifact_sha256` hashes decoded
binary bytes, rather than the hexadecimal file. `stdout`, `stdin`, and `args`
describe the process contract. Successful checks and executions produce no
stderr and exit with status 0.

| Fixture | Wire version | Historical producer revision | Bytes | Expected stdout |
| --- | --- | --- | --- | --- |
| `collections-v1` | 1 | `044b02dbbeca710ccd28f2f65745361e7b40447f` | 1862 | `5\n50\n` |
| `host-arguments-v2` | 2 | `b381fd3178ab78115317a4e37796e72b5401ed1e` | 502 | `2\n` |

The v1 source is an exact copy of `examples/collections.larp` at its producer
revision. It exercises collection callbacks and ordering. The v2 source uses
the host-input intrinsic `args()` and is run with program arguments `unc bro`.
This distinguishes v2 functionality from a header-only change to v1 bytes.

## Recorded production and verification

Both producers were built on 2026-10-19 using their complete tracked source
archives, exported with `sh tools/git-local.sh archive --format=tar REVISION`.
Only tracked files from those commits were extracted; private notes and the
current working tree were not copied into either producer. Independent source
and target directories were retained under the ignored path
`artifacts/historical-bytecode/version-policy-baseline/{v1,v2}/`.

For each historical source tree the build command was:

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=SEPARATE_TARGET \
    cargo build --package larp-cli --locked --offline
```

The actual toolchain was Arch Linux Rust 1.99.0, rustc commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, host
`x86_64-unknown-linux-gnu`, and Cargo 1.99.0. The complete compiler identity,
producer CLI hash, and tracked source archive hash are recorded per case in the
manifest. The identical `0.1.0-dev` version strings do not identify producer
revisions; the full Git revisions do. Producer CLI hashes describe these
particular local builds and do not promise equality across build environments.

Each producer ran `build SOURCE -o ARTIFACT` from a dedicated directory using
the source basename recorded in the manifest. Its unmodified output was checked
for the `LARPBC\r\n` magic and expected little-endian version, then encoded as
hexadecimal. No artifact header, opcode, source map, or other byte was edited.

Both the historical producer and the current CLI successfully executed:

```sh
larp check ARTIFACT
larp run ARTIFACT
larp run ARTIFACT --gc-stress
```

For `host-arguments-v2`, both run commands also received `-- unc bro`.
All checks returned status 0, exact manifest stdout, and empty stderr. Local
command records remain in the ignored
`artifacts/historical-bytecode/version-policy-baseline/verification.json`.

Keep existing fixture bytes and hashes fixed when changing the compiler.
Add a separately identified historical fixture when introducing another wire
version or an additional compatibility contract. Regenerating these files with
the current encoder would remove the historical evidence they are meant to
preserve.
