# Source formatter validation

Date: 2026-10-18. Scope: the M6 source formatter and CLI integration. This is
local acceptance evidence, not the completed developer-preview release gate.

## Implemented contract

`larp-format` uses the real parser plus AST annotations over a lossless lexical
stream. The concrete delimiter tree preserves parentheses, literal spellings,
comments, declaration order, and class-member order. Generic brackets, comparison
operators, prefix operators, calls, and lambda blocks receive syntax-aware layout.
Every result is reparsed and its ordered raw tokens/comments compared with input
before it can be returned or written. Lexing for normal compilation does not
allocate comment records.

The CLI provides `fmt SOURCE`, `fmt SOURCE --check`, and `fmt SOURCE --stdout`.
Formatting requires parseable source, not successful type/effect checking. The
write mode prepares the complete result before publication, follows source
symlinks, preserves permissions, and checks current bytes before a sibling-file
rename. An unchanged file is not rewritten. The check/preview modes never write.
Parse, lexical, resource, and preservation failures do not publish partial output.

See [formatter policy](formatter.md) for exact layout choices, diagnostics,
logical budgets, and filesystem limitations. In particular, the byte check is
not a filesystem compare-and-swap, and formatting does not preserve inode identity
or promise crash durability.

## Evidence

Formatter unit tests pin representative output and cover generic/function types,
comparison precedence, prefix signs, explicit parentheses, nested calls and lists,
typed lambdas, blocks/continuations, and source/member order. Public API tests
insert line, CRLF line, and block comments at every token boundary in templates
covering roles, classes, extensions, collections, expected errors, scenario
operations, lazy bindings, suppression, and loops. Every generated case must
parse, format successfully, retain exact raw lexemes in order, and be idempotent.
The tests also format existing examples and all 77 conformance files without
changing those files.

Lossless lexer tests cover Unicode, CRLF, comment markers inside strings,
comments between paired keywords/operators, raw numeric separators, unterminated
comments/strings, invalid escapes, and reconstruction from spans and whitespace
gaps. A deterministic 1,000-stream check compares compiler and lossless lexical
acceptance, tokens, diagnostics, and positions. The compiler scanner is checked
to retain zero comment records.

Review found and fixed these concrete cases:

- A single pending-break flag treated a parenthesized statement after a semicolon
  as a continuation. Break reasons now distinguish required statement/list breaks
  from closing-block continuations; nested array and typed-lambda arguments have
  explicit spacing/break regressions.
- A 16 MiB output cap alongside a 4 MiB input cap could return text that the next
  formatter pass rejected. Both caps are now 4 MiB. A small, deeply indented source
  with 34,000 statements exercises output expansion failure without partial output.
- Token counting originally followed AST construction. The formatter now checks
  the lexical token/comment budget before constructing an AST.

CLI tests reject oversized UTF-8 input before decoding/source indexing and
exercise preview/check/write exit codes and file contents, second-pass
stability, semantic-error source, lexical/syntax failure, artifact rejection,
invalid option combinations, source permissions, and symlink retention. A
publication test injects a changed source before rename and checks that the new
edit survives; a failed directory rename leaves no temporary file.

## Final local checks

The following checks passed:

- Formatting and workspace all-targets Clippy with warnings denied.
- 346 workspace tests, including 19 formatter tests, 42 syntax tests, and the
  CLI publication/mode tests.
- 117 compiler tests in release mode.
- Corpus inventory: 77 active cases, 0 pending.
- All 77 cases through source `check` and `run`.
- All 77 cases through the combined `--formatted --artifacts --gc-stress` path:
  temporary formatting, idempotence check, real compilation/execution, artifact
  build/loading/verification, and stress collection. Passing cases retain exact
  stdout/inspection output; failing cases retain their primary code and a valid
  source location. Artifact diagnostics match their corresponding formatted source.
- Git whitespace validation before commit.

CI includes the combined corpus path. Local results alone do not establish a
hosted run. Deterministic generated tests are not sustained coverage-guided
fuzzing. Whole-process resource measurements, host APIs, broader reference docs,
platform/distribution work, and licensing remain developer-preview work.
