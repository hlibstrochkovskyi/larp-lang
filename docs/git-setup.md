# Git setup in the managed workspace

The ordinary `.git` directory was initialized on `main`, but the environment
mounts that path read-only. `git add` failed while creating `.git/index.lock`.
No standard-repository commit could be written there. This is a filesystem
restriction, not a request for maintainer approval.

The foundation history is instead stored in the ignored, writable
`.git-foundation/` directory. Use the helper from this checkout:

```sh
sh tools/git-local.sh status
sh tools/git-local.sh log --oneline
sh tools/git-local.sh diff
```

The helper explicitly selects the alternate Git directory and this worktree.
Plain `git` continues to select the protected `.git`; do not confuse its unborn
branch with the alternate repository's saved history. The alternate repository
uses the maintainer's existing author/committer configuration, with no assistant
credit. The maintainer-selected `origin` is
`https://github.com/hlibstrochkovskyi/larp-lang.git`; `main` tracks `origin/main`.
The 2026-10-19 read-only audit confirmed 12 project commits through `cc1354d`,
236 tracked files, a readable index exactly matching HEAD, clean tracked files,
and successful `git fsck --full`. Local `main`, `origin/main`, and the hosted
GitHub branch all matched `cc1354d3c294c47565adedeaffb9ca56fa5b1d7e` at that audit.
These are dated observations; later development can change the head and counts.

In contrast, ordinary `.git` had no index, no `origin`, and an unborn `main`.
Its `refs/agents/...` contained host-generated checkpoints, not the published
project history. Consequently plain `git status --short` listed the project as
untracked. Do not stage everything or create a new initial commit to resolve
that symptom. Do not delete checkpoint objects or replace either metadata tree.
Inspect the selected Git directory before changing history:

```sh
git rev-parse --git-dir
git status --short --branch
sh tools/git-local.sh rev-parse --git-dir
sh tools/git-local.sh status --short --branch
sh tools/git-local.sh remote -v
sh tools/git-local.sh log -5 --oneline
```

Only the helper's status describes the saved project history in this managed
checkout. A standard clone elsewhere uses its ordinary `.git`; this alternate
metadata arrangement is local tooling, not a language/build requirement.

A portable `artifacts/larp-foundation.bundle` is regenerated from `main` after
implementation commits. Both the bundle and alternate metadata are ignored by Git.
To restore a normal repository in a new writable location:

```sh
git clone --branch main /absolute/path/to/Larp/artifacts/larp-foundation.bundle Larp-restored
```

This creates ordinary Git metadata in the new checkout; it does not contain the
private brief or local build output. The clone's initial origin is the local
bundle. In a restored checkout, use the maintainer-selected hosted destination:

```sh
git remote set-url origin https://github.com/hlibstrochkovskyi/larp-lang.git
```

Terminal network access and Git authentication are separate from the connected
GitHub tool. Configuring a remote alone does not establish that a push succeeded.
Always inspect the hosted branch head before claiming publication.
Alternatively, outside the managed read-only restriction, the maintainer can
import the saved history into the original fresh `.git`. Do not overwrite an
existing repository's metadata to do so.
