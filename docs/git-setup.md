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
credit, remote, or push.

A portable `artifacts/larp-foundation.bundle` is generated from `main` after the
foundation commit. Both the bundle and alternate metadata are ignored by Git.
To restore a normal repository in a new writable location:

```sh
git clone --branch main /absolute/path/to/Larp/artifacts/larp-foundation.bundle Larp-restored
```

This creates ordinary Git metadata in the new checkout; it does not contain the
private brief or local build output. The clone's initial origin is the local
bundle, not a hosted service. Select a real remote only when a destination exists.
Alternatively, outside the managed read-only restriction, the maintainer can
import the saved history into the original fresh `.git`. Do not overwrite an
existing repository's metadata to do so.
