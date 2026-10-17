# Project instructions

- Speak with the maintainer in Russian. Write repository documentation, code,
  comments, diagnostics, test descriptions, and commit messages in English.
- Do not add an assistant as an author, contributor, or commit co-author. Use the
  maintainer's existing Git identity. Do not invent a license or remote repository.
- Keep `Ideas-not-for push!/` private and untracked. Do not copy its contents into
  public history. Public design documents must be independent English analyses.
- Read `docs/spec/v0.1.md`, `docs/architecture.md`, and `docs/roadmap.md` before
  compiler work. The specification is a proposal, not a claim of implementation.
- Keep implementation status explicit. Never report a fixture as a passing LARP
  program until the real compiler and VM have executed it.
- LARP is an independent statically typed language. Do not translate programs to
  Python/JavaScript or use host `eval` to implement it.
- Preserve source spans and structured diagnostics across compiler phases.
- Do not bypass checks through aliases, callbacks, role dispatch, or bytecode.
- Prefer small vertical slices with executable acceptance criteria. Add crates
  when a boundary has real code; avoid placeholder compiler subsystems.
- Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked
  --offline -- -D warnings`, `cargo test --workspace --locked --offline`, and
  `python3 tools/check_corpus.py`, and `python3 tools/run_conformance.py` for
  changes to the compiler or foundation.
- Run `git diff --check` before committing. Keep commits focused and in English.
