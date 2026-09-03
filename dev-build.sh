#!/bin/bash
#
# dev-build.sh - run cargo against local sibling checkouts, reliably.
#
# Why this exists: this repo depends on sibling bridge crates as *git*
# dependencies, with gitignored [patch] overrides in .cargo/config.toml
# redirecting them to local checkouts during development. That combination
# is a trap for bare cargo:
#
#   * When the local crate's version EQUALS the locked one, any resolving
#     cargo command (build/test/check/run) applies the patch immediately and
#     silently REWRITES Cargo.lock with local-path entries that must never
#     be committed (CI has no sibling checkouts).
#   * When the versions differ, the patch is silently IGNORED and you build
#     the GitHub revisions instead of your local edits.
#
# Either way bare cargo does the wrong thing, so always go through this
# script. It keeps two lockfiles and swaps them around the cargo call:
#
#   Cargo.lock       committed lock, pinned to git sources (CI truth)
#   .cargo/dev.lock  local-only lock, resolved with the patches applied
#
# and verifies every patched crate in the dependency graph actually resolved
# to a local path, failing loudly if not. The committed Cargo.lock is never
# touched.
#
# Config discovery: cargo merges .cargo/config.toml from every *ancestor* of
# the invocation directory, so the overrides that apply here are not
# necessarily next to this script. In a git worktree under
# .claude/worktrees/<name>/ there is no local .cargo/ at all, yet the main
# checkout's config still patches the build. Looking only beside the script
# made this script fall through to a bare invocation in exactly that case --
# the one place bare cargo silently corrupts a lockfile, and with --ci not
# even reaching the guard that would have said so. So we walk up the way
# cargo does and manage whichever config we find. Lockfiles stay per-worktree
# (Cargo.lock is), while the config, and therefore the --ci move-aside, may be
# shared: don't run two --ci builds against the same config concurrently.
#
# Usage:
#   ./dev-build.sh                  # cargo build, against local checkouts
#   ./dev-build.sh test             # cargo test, against local checkouts
#   ./dev-build.sh build --release  # any cargo subcommand + args
#   ./dev-build.sh --ci test        # CI-parity: patches disabled, committed
#                                   # lock's git pins, lock rewrite guarded
#   ./dev-build.sh --exec wasm-pack build ...
#                                   # run an arbitrary cargo-invoking command
#                                   # under the same lockfile protection
#   ./dev-build.sh --workspace wasm test
#                                   # protect wasm/Cargo.lock instead of the
#                                   # root one -- wasm/ is a separate workspace
#                                   # that still inherits this config's patches
#
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$SCRIPT_DIR"

# Both spellings cargo accepts, newest first.
CONFIG_NAMES=(config.toml config)

# Nearest ancestor .cargo/ config, starting at $1, that actually carries
# [patch.] overrides. Filtering on [patch.] during the walk matters: a
# ~/.cargo/config.toml without one must not stop the search.
find_patch_config() {
    local dir=$1 name
    while :; do
        for name in "${CONFIG_NAMES[@]}"; do
            if [[ -f $dir/.cargo/$name ]] && grep -q '^\[patch\.' "$dir/.cargo/$name"; then
                printf '%s\n' "$dir/.cargo/$name"
                return 0
            fi
        done
        if [[ $dir == / ]]; then
            return 1
        fi
        dir=$(dirname "$dir")
    done
}

# Nearest ancestor marker left by an in-flight (or crashed) --ci run.
find_disabled_config() {
    local dir=$1 name
    while :; do
        for name in "${CONFIG_NAMES[@]}"; do
            if [[ -f $dir/.cargo/$name.ci-off ]]; then
                printf '%s\n' "$dir/.cargo/$name.ci-off"
                return 0
            fi
        done
        if [[ $dir == / ]]; then
            return 1
        fi
        dir=$(dirname "$dir")
    done
}

ci_mode=""
if [[ ${1:-} == --ci ]]; then
    ci_mode=1
    shift
fi

# Which crate's lockfile to protect. `wasm/` is its own workspace, but cargo
# config discovery walks upward, so it inherits this config's [patch] overrides
# and its lock is exposed to exactly the same rewrite. Default: the root.
workspace=.
if [[ ${1:-} == --workspace ]]; then
    shift
    if [[ $# -eq 0 ]]; then
        echo "dev-build: --workspace needs a directory" >&2
        exit 2
    fi
    workspace=${1%/}
    shift
    if [[ ! -f $workspace/Cargo.toml ]]; then
        echo "dev-build: no Cargo.toml in $workspace" >&2
        exit 2
    fi
fi

LOCK=$workspace/Cargo.lock
# The command runs *in* the selected workspace, so a plain `--workspace wasm
# clippy` lints that crate rather than the root one. Paths in the caller's
# arguments are therefore relative to it. Lock and config paths below stay
# relative to the repo root, which is where this script keeps its own cwd.
run() { ( cd "$workspace" && "${runner[@]}" "$@" ); }
slug=$([[ $workspace == . ]] && echo dev || echo "dev-$(echo "$workspace" | tr / -)")
DEV_LOCK=.cargo/$slug.lock
CI_LOCK_STASH=.cargo/$slug.ci.swap

# By default the arguments are a cargo subcommand; --exec instead runs the rest
# as a command of its own (wasm-pack, cargo-nextest, ...). Those shell out to
# cargo themselves, so they need the same lock swap that a direct call gets.
# `command` is a no-op prefix, which keeps the runner a non-empty array -- an
# empty "${arr[@]}" is an unbound-variable error under set -u in bash 3.2.
runner=(cargo)
if [[ ${1:-} == --exec ]]; then
    shift
    if [[ $# -eq 0 ]]; then
        echo "dev-build: --exec needs a command to run" >&2
        exit 2
    fi
    runner=(command)
elif [[ $# -eq 0 ]]; then
    set -- build
fi

CONFIG=$(find_patch_config "$SCRIPT_DIR") || CONFIG=""

# No local patch overrides anywhere above us: behave exactly like a direct
# invocation.
if [[ -z $CONFIG ]]; then
    stray=$(find_disabled_config "$SCRIPT_DIR") || stray=""
    if [[ -n $stray ]]; then
        echo "dev-build: ERROR: $stray exists." >&2
        echo "dev-build: another --ci run has the patch overrides moved aside, or one" >&2
        echo "dev-build: crashed before restoring them. Wait for it, or rename that file" >&2
        echo "dev-build: back to ${stray%.ci-off} if nothing else is running." >&2
        exit 1
    fi
    run "$@"
    exit $?
fi

CONFIG_DIR=$(dirname "$CONFIG")
CONFIG_OFF="$CONFIG.ci-off"

# We have to be able to move the config aside (--ci) and read the patch list
# (dev). Falling back to bare cargo here is not safe: cargo would still apply
# these overrides and rewrite the lockfile with local-path entries.
if [[ ! -w $CONFIG || ! -w $CONFIG_DIR ]]; then
    echo "dev-build: ERROR: $CONFIG carries [patch] overrides but is not writable" >&2
    echo "dev-build: (neither is $CONFIG_DIR), so this script cannot disable or" >&2
    echo "dev-build: inspect them. Refusing to run bare cargo, which would apply the" >&2
    echo "dev-build: patches and rewrite the lockfile with local-path entries." >&2
    exit 1
fi

if [[ $CONFIG_DIR != "$SCRIPT_DIR/.cargo" ]]; then
    echo "dev-build: patch overrides from $CONFIG" >&2
fi

# --- CI-parity mode: disable the patches, build with the committed lock ---
if [[ -n $ci_mode ]]; then
    lock_before=""
    [[ -f $LOCK ]] && lock_before=$(cksum < "$LOCK")
    mv "$CONFIG" "$CONFIG_OFF"
    restore_ci() { [[ -f $CONFIG_OFF ]] && mv "$CONFIG_OFF" "$CONFIG"; }
    trap restore_ci EXIT
    run "$@"
    if [[ -n $lock_before && $(cksum < "$LOCK") != "$lock_before" ]]; then
        echo "dev-build: NOTE: Cargo.lock was re-resolved during this CI-parity run." >&2
        echo "dev-build: review 'git diff Cargo.lock' — internal crates must keep their" >&2
        echo "dev-build: source = \"git+https://...\" lines before committing." >&2
    fi
    exit 0
fi

# --- dev mode: swap in the dev lock, build against local checkouts ---

# Lockfiles are per-manifest, so they always live beside *this* checkout even
# when the config above is shared with the main worktree.
mkdir -p .cargo

# Crate names the config patches to local paths.
patched=$(sed -n 's/^\([A-Za-z0-9_-]*\) *= *{ *path *=.*/\1/p' "$CONFIG")

swapped=""
restore() {
    if [[ -n $swapped ]]; then
        [[ -f $LOCK ]] && mv "$LOCK" "$DEV_LOCK"
        [[ -f $CI_LOCK_STASH ]] && mv "$CI_LOCK_STASH" "$LOCK"
    fi
}
trap restore EXIT

# If the committed (CI) lock is tracked, set it aside and use the dev lock;
# cargo re-creates the dev lock from scratch if it doesn't exist yet, and a
# fresh resolve does honor the config patches.
if git ls-files --error-unmatch "$LOCK" >/dev/null 2>&1; then
    swapped=1
    mv "$LOCK" "$CI_LOCK_STASH"
    [[ -f $DEV_LOCK ]] && mv "$DEV_LOCK" "$LOCK"
else
    # Nothing to protect yet, so this run writes $LOCK directly -- and with the
    # patches active that lock records local paths. Fine to build with, wrong to
    # commit: CI has no sibling checkouts. Say so, because the resulting file
    # looks perfectly ordinary.
    echo "dev-build: NOTE: $LOCK is not tracked by git, so it is being written" >&2
    echo "dev-build: with the local [patch] applied. Do not commit it as-is --" >&2
    echo "dev-build: regenerate the committed lock with:" >&2
    echo "dev-build:   mv $LOCK $DEV_LOCK && ./dev-build.sh --ci${workspace:+ --workspace $workspace} check" >&2
fi

# True when every patched crate that appears in the lock is path-resolved
# (path-resolved entries are the only ones without a `source =` line).
verify() {
    local ok=0 crate
    for crate in $patched; do
        grep -q "^name = \"$crate\"\$" "$LOCK" 2>/dev/null || continue
        if grep -A2 "^name = \"$crate\"\$" "$LOCK" | grep -q '^source ='; then
            echo "dev-build: $crate still resolves to a remote source" >&2
            ok=1
        fi
    done
    return $ok
}

run "$@"

if [[ -f $LOCK ]] && ! verify; then
    # Stale dev lock from before the patches existed; it is disposable —
    # discard it and re-resolve fresh, which applies the patches.
    echo "dev-build: discarding stale dev lock and re-resolving..." >&2
    rm "$LOCK"
    run "$@"
    verify || {
        echo "dev-build: ERROR: patched crates still resolve to remote sources." >&2
        echo "dev-build: check that the sibling checkouts in $CONFIG exist." >&2
        exit 1
    }
fi

for crate in $patched; do
    if grep -q "^name = \"$crate\"\$" "$LOCK" 2>/dev/null; then
        echo "dev-build: ✓ $crate → local checkout"
    fi
done
