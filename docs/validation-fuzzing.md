# Coverage-guided robustness baseline

Date: 2026-10-19. Scope: short, local instrumented campaigns against production
lexer/parser, formatter, compiler, artifact loader, and mandatory bytecode verifier.
This is executed evidence, not proof of complete conformance or input coverage.

## Configuration and oracles

The independent [fuzz workspace](../fuzz/README.md) pins `libfuzzer-sys` 0.4.13
and its registry dependency lock. Tooling was `cargo-fuzz` 0.13.2 and Rust
`nightly-2026-10-17` (`1.101.0-nightly`, commit `5d8fec066`), on Linux x86-64.
The production workspace retains zero third-party Rust dependencies. Local
tooling was installed in ignored storage without replacing the system compiler.

Targets were built with default AddressSanitizer, coverage instrumentation,
debug assertions and overflow checks, using 16 codegen units. No production
check is removed by `cfg(fuzzing)`. Each target admits at most 65,536 input bytes.
Source targets accept UTF-8; the loader accepts binary input.

The shared `larp-fuzz-support` oracles check:

- Frontend: lexer/parser failures have registered diagnostics and valid source
  spans; successful tokens/comments are ordered and non-overlapping, with one EOF.
- Formatter: successful output parses, preserves exact token/comment spellings
  and order, and formats identically a second time. Internal invariant failures
  are defects, not expected source errors.
- Compiler: normal source diagnostics are allowed; compiler-generated invalid
  bytecode is a defect. Successful artifacts survive mandatory verification,
  preserve disassembly/source maps, and have deterministic canonical bytes.
  Persistence resource limits remain legitimate rejection reasons.
- Loader: raw bytes and, when different, an outer-payload-length-adjusted copy go
  through the actual bounded decoder and verifier. Accepted modules must re-encode
  canonically. Repairing length never skips section, type, or effect validation.

The targets do not execute arbitrary VM programs: fuel does not bound memory
allocated by an instruction. Runtime behavior is covered separately by corpus,
adversarial bytecode, GC stress, and [resource measurements](validation-resources.md).

## Executed campaigns

Each successful campaign used `-seed=49 -max_len=65536 -max_total_time=60
-timeout=5 -rss_limit_mb=1024 -print_final_stats=1`. Actual wall duration was about
61 seconds per target, including initialization and shutdown.

| Target | Initial inputs | Executed units | Final coverage feedback | Final feature feedback | Reported peak RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Frontend | 1,448 | 205,471 | 2,125 | 9,343 | 444 |
| Formatter | 92 | 296,583 | 3,176 | 12,736 | 455 |
| Compiler | 92 | 329,357 | 10,449 | 33,082 | 479 |
| Artifact | 292 | 646,994 | 4,265 | 15,827 | 479 |

Total: **1,478,405 executed units**, four zero exit statuses, no host timeouts,
and no failure inputs from these successful campaigns. This does not imply that
every unit parsed or compiled: invalid source/binary rejection is expected.
The counters are libFuzzer feedback, not percentages of source lines or branches.

An earlier frontend campaign executed 213,918 units, then LeakSanitizer failed
at shutdown because this managed host does not permit its required ptrace
operation. Its nonzero exit is retained as an unsuccessful run. The empty
failure input written at that shutdown is an environment failure artifact,
not a LARP regression. No system ptrace restriction was changed.

For the four successful runs, only leak checking was explicitly disabled with
`ASAN_OPTIONS=detect_leaks=0`; AddressSanitizer memory-access checks remained on.
These runs supply no leak-check evidence. The default runner and hosted workflow
leave leak checking enabled. A host sanitizer failure is never a passing gate.

The successful frontend run reused the corpus grown by the earlier unsuccessful
campaign. The other three began from the initial public fixture seeds. Initial
source inventories included one empty seed from a directory marker; the final
seed tool excludes markers and now produces 91 source inputs per target and 292
binary inputs on an empty output directory. Earlier evidence is preserved rather
than silently relabeled with the corrected seed inventory. Corpus hashes/counts
are recorded at each actual campaign start.

Raw logs, executable/corpus/lock hashes, commands, counters, and outcomes are
retained in ignored `artifacts/fuzz-campaign/`; the failed sanitizer run remains
in `artifacts/fuzz-campaign-lsan/`. All successful log hashes were independently
checked against the summary. The current runner refuses nonempty report
directories, preventing old findings from being attributed to a new campaign.
The seed tool likewise requires empty output and checks public fixture compilation
contracts rather than treating an internal compiler error as an ordinary seed.
The final seed-tool/runner revisions were exercised again on a fresh output:
four shorter smoke campaigns all exited successfully and retained their logs
separately under `artifacts/fuzz-final-smoke/`.

## Stable checks and remaining limits

Four stable oracle tests passed: public seeds with 168 deterministic source
mutations, 384 mutations of real compiler artifacts, invalid UTF-8/size admission,
and saved-regression replay. The saved finding inventory is currently empty;
the deterministic mutations are not described as coverage-guided campaigns.
The complete workspace tests, formatting, Clippy with denied warnings, compiler
release tests, and corpus checks passed locally. All 84 active corpus cases
passed both ordinary execution and the combined formatter/artifact/GC-stress path.

The new hosted workflow runs on relevant main-branch pushes, weekly, and by
manual dispatch; normal CI replays stable oracles and performs a one-sample
resource smoke check. Local success does not establish hosted workflow success.

This baseline is intentionally short. Public source fixtures are mostly below
one KiB; the observed dynamic mutation length remained near that scale. A 64 KiB
maximum does not establish meaningful exploration of maximum token counts,
nesting depth, type expansion, or verifier work limits. Existing targeted limit
tests cover particular witnesses, not all combinations. Longer independent
campaigns, explicit near-boundary seeds, diverse seeds/toolchains, allocation
profiling, and leak checking on a capable host remain useful follow-up work.
Resource-limit findings require classification: a configured timeout or RSS
limit is not automatically a semantic defect or a host capability violation.
