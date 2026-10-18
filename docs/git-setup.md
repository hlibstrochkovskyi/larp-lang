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
The remote head was inspected through the connected GitHub tool and matched
local commit `8eeef7c` before the formatter slice.

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
