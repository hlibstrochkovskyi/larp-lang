# Minimized public regressions

Place each finding in the directory named for its oracle: `frontend/`,
`formatter/`, `compiler/`, or `artifact/`. Stable workspace tests replay every
regular file in those directories. Use `.larp` for source and `.bin` for binary
inputs. Do not include private design notes, credentials, or generated host input.

No coverage-guided finding has been recorded yet. Deterministic mutation tests
are separate evidence and must not be described as a fuzz campaign.
