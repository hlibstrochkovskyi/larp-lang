#!/usr/bin/env sh
# Use writable Git metadata when the managed environment protects .git.
set -eu
larp_repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec git --git-dir="$larp_repo_root/.git-foundation" --work-tree="$larp_repo_root" "$@"
