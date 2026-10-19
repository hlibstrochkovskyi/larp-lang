# CLI diagnostic protocol

The CLI defaults to human-readable diagnostics. Select JSON Lines with one
global option before the command:

```sh
larp --diagnostic-format=json check main.larp
larp --diagnostic-format=json run program.larpbc --gc-stress
larp --diagnostic-format=text run main.larp
```

The option applies to `check`, `build`, `run`, `dump`, and `fmt` failures.
Exit codes keep their ordinary meaning: 0 for success, 1 for compilation/runtime
or file/tooling failure, 2 for invalid CLI usage. Successful commands emit no
completion record. Stdout keeps its usual contents, including program output,
formatted source, dumps, explanations, help, and version text. A value after
`run ... --` is a program argument, even if it spells this global option.

In JSON mode, stderr contains UTF-8 JSON objects separated by physical LF.
Newlines, tabs, quotes, backslashes, and all U+0000–U+001F controls inside strings
are escaped. Other Unicode characters are preserved. Consumers must split on
LF rather than all Unicode line separators. Every record has `schema_version: 1`
and a `kind`. Consumers should ignore additional object fields and dispatch by
kind; a human message is not an identifier or a machine type representation.
The protocol describes completed writes, not recovery from a failed stderr sink.

## Source diagnostics

Compiler, formatter, and VM diagnostics use this shape:

```json
{
  "schema_version": 1,
  "kind": "diagnostic",
  "severity": "error",
  "code": "LARP-T041",
  "title": "MOGGED: implicit integer narrowing",
  "message": "Expected mf, found unc",
  "primary": {
    "source_id": 0,
    "path": "main.larp",
    "start": 45,
    "end": 50,
    "start_location": {"line": 3, "column": 16},
    "end_location": {"line": 3, "column": 21},
    "text": "large",
    "message": "Expected mf, found unc"
  },
  "related": [],
  "expected_type": "mf",
  "found_type": "unc",
  "help": "Use a matching integer type, or ascend<mf>(value) and handle the result with .cope(handler)"
}
```

This is a schema illustration, not an execution fixture. Each actual primary or
related label contains its own source ID, path, half-open UTF-8 byte offsets,
one-based Unicode-scalar line/column coordinates, source slice, and label message.
Tabs count as one column. End coordinates identify the exclusive end, including
EOF or a later line for a multiline span. Source IDs identify files within one
compilation; they are not stable project-wide identities.

Each related label resolves against its own source. If its source is unavailable,
the ID and offsets remain, while path, locations, and source text are null.
Expected/found type strings and help are nullable. Severity is `error` or
`warning`; stable codes remain the diagnostic identity. Titles, messages,
labels, and help can improve without changing the code. Diagnostics loaded from
an artifact use its embedded original sources, even after the input file moves
or the source is deleted.

## Other stderr events

These events have `schema_version`, `kind`, nullable `code`, and `message`:

| Kind | Meaning |
| --- | --- |
| `inspection` | A UTF-8 chunk of `palantir` output; code is null |
| `cli_error` | File/argument/output failure without a source diagnostic; code is null |
| `artifact_error` | Artifact encode/load failure; code is a `LARP-B...` identifier |
| `format_needed` | `fmt --check` found a formatting difference; code is null |

Concatenating inspection messages in order reconstructs the ordinary stderr
report exactly. Record/chunk boundaries are not a report or row API. Inspection
is emitted as execution proceeds and is not rolled back on a later runtime
failure. A subsequent fatal diagnostic follows those inspection records.
`sybau` continues to suppress only ordinary `yap`; it does not hide diagnostics
or inspection. No JSON record replaces program stdout.

## Acceptance checks

The [conformance harness](../tools/run_conformance.py) exercises JSON mode as
well as ordinary text mode. All runtime fixtures identify a unique source slice;
the harness independently calculates its exact byte offsets and coordinates and
compares the diagnostic's complete primary range. It repeats that contract after
formatting and artifact loading when those modes are selected. Successful
scenario programs also reconstruct inspection output from JSON chunks.

The [CLI tests](../crates/larp-cli/tests/cli.rs) cover expected/found types, help,
related declarations, formatter/CLI/artifact errors, a path containing quotes,
newline and Unicode, CRLF/tab/scalar coordinates, and loaded diagnostics after
source deletion. The [renderer tests](../crates/larp-diagnostics/src/json.rs)
cover controls, multiple source IDs, and missing source maps.

On 2026-10-19, local formatting and Clippy checks passed, as did 378 workspace
tests and 129 compiler release tests. All 85 cases passed in normal, GC-stress,
loaded-artifact stress, and formatted/loaded-artifact stress modes, including the
additional JSON executions. Eight reference examples passed 40 VM executions
and 113 CLI commands. Evidence remains in ignored
`artifacts/reference-json-diagnostics/`.

A negative control supplied the overflow fixture's entire `return` statement
as an intentionally wrong expected span. The real JSON execution was rejected
with a primary-range mismatch, confirming that location presence alone cannot
satisfy the strengthened check. That observation and the two open semantic
reproductions remain in `artifacts/preview-audit-json/evidence.json`.
These are local results; earlier hosted jobs do not validate this new slice.
