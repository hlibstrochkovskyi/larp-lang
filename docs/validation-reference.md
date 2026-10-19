# Documentation example validation

The practical reference and main README now have separate executable contracts.
The historical reference run below retains its original scope and counts.

## README pitch example

The main [README](../README.md) contains one complete LARP program, also stored
as [examples/pitch-deck.larp](../examples/pitch-deck.larp). The harness checks that
they are identical before executing [readme-cases.json](readme-cases.json).

```sh
python3 tools/run_reference.py --document README.md --contracts docs/readme-cases.json --output artifacts/readme-validation
```

This runs the same source/check/format/artifact contracts as the reference suite:
15 CLI commands, including five VM executions, exact stdout `49\n20\nfr\n`, and
the declared `PALANTIR [UnicornPitch]` stderr report. CI includes this command.
The example was also executed through the unchanged published `0.1.0-preview.1`
binary with `--bin`, including normal/stress source and loaded artifacts and
formatted source. This proves the snippet works on that executable even though
the example file was added after the release archive was frozen.

Use `--document` and `--contracts` together to select another document and its
ordered contracts; `--bin` avoids rebuilding an existing CLI. Output directories
must be new or empty. Document, contract, and binary hashes are retained with
actual command results.

## Historical practical reference run

Date: 2026-10-19. Scope: the eight complete LARP code blocks in
[the practical reference](reference.md), not every possible program described by
its prose or the complete specification.

Built the current CLI with `cargo build -p larp-cli --locked --offline`. A local
Python extraction driver read all eight `larp` fences directly from the Markdown
and wrote them into ignored `artifacts/reference-check/`. It ran 113 CLI commands,
including 40 successful VM executions. Each command received explicit stdin,
captured stdout/stderr, and a ten-second process timeout.

Each extracted sample passed:

- Source `check`, then `run` in normal and stress GC modes.
- Formatter stdout preview, formatted-source `--check`, exact formatter
  idempotence, and formatted-source execution under stress GC.
- Artifact build and independent artifact `check`, then artifact execution in
  normal and stress GC modes. Generated source files remained present.
- Artifact disassembly, source typed-IR dump, and the formatter's default write
  command on the generated formatted copy.

The driver also executed `explain LARP-T041`. Every command exited zero; all 40
VM executions matched exact stdout and stderr. No code block was treated as a
passing program merely because parsing or formatting succeeded.

| Sample in document order | Exact stdout, with LF escaped |
| --- | --- |
| Numbers and loops | `79\n0\n1\n2\nfr\n` |
| Functions, captures, callbacks | `12\n14\n59\n` |
| Shared arrays and fixed copies | `1\n2\n` |
| Options, results, checked conversion | `20\n49\n20\n` |
| Objects, role view, bound method, extension | `12\n` |
| Scenario with final-field selection | `20\n3\n` |
| Lazy capture and dynamic suppression | `8\n10\n` |
| Arguments, present/blank line, EOF | `demo\n--literal\nhello\n\nEOF\n` |

The input sample received program arguments `demo` and `--literal` after the CLI
separator, and fresh stdin `hello\n\n` on every execution. Other samples received
explicit EOF and no program arguments. Only the scenario emitted stderr, checked
against this exact report:

```text
PALANTIR [Trial]
PROPERTY | REALITY | PERFORMED | LAST CLAIM | SELECTED
valuation | 20 | 1000000000 | 1000000000 | false
aura | 0 | 3 | - | true
```

The original local driver and evidence remain in ignored
`artifacts/reference-check/`. The checked-in `tools/run_reference.py` now extracts
the same fences and checks `docs/reference-cases.json` contracts; it was executed
independently and also passed all 113 commands and 40 VM executions. Run:

```sh
python3 tools/run_reference.py
```

This command builds the real CLI offline and is included in normal CI. It
requires a fresh output directory; use `--output artifacts/reference-next` for
another run. Sources, formatted copies, loaded artifacts, actual input/output,
binary/document/contract hashes, and command outcomes are retained under ignored
`artifacts/reference-validation/`. Nothing is inferred from syntax-only success.
This validation does not establish a release, platform support, distribution
license, performance guarantee, or full language conformance.
