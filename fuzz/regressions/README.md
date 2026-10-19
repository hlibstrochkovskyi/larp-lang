# Minimized public regressions

[Robustness checks](../README.md) · [Contributing](../../CONTRIBUTING.md)

This directory stores small, reviewed inputs for defects found by the
coverage-guided oracles. Run the stable replay from the repository root:

```sh
cargo test -p larp-fuzz-support --locked --offline
```

Place each finding in the directory named for its oracle: `frontend/`,
`formatter/`, `compiler/`, or `artifact/`. Stable workspace tests replay every
input file in those directories; `.gitkeep` and `README.md` are markers and are
skipped. Use `.larp` for source and `.bin` for binary inputs. Entries must be
regular files, not symlinks, and at most 64 KiB.

For each new regression, record the cause and expected behavior here alongside
the production fix. Do not include private design notes, credentials, or
generated host input.

No coverage-guided finding has been recorded yet. Deterministic mutation tests
are separate evidence and must not be described as a fuzz campaign.
