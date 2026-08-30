#!/bin/bash
#
# wasm-build.sh - build the WebAssembly package.
#
# Wraps wasm-pack with the two things this crate needs and cargo cannot infer:
#
#   * --cfg getrandom_backend="wasm_js". printpdf and lopdf depend on getrandom,
#     which has no entropy source on wasm32-unknown-unknown. The wasm_js feature
#     (see Cargo.toml) routes it at crypto.getRandomValues, but getrandom 0.3
#     also requires this cfg before it will use that backend -- without it the
#     build still succeeds and then fails at runtime, on the first render.
#     It lives here rather than in .cargo/config.toml because that file is
#     gitignored (it holds local-only [patch] overrides).
#
#   * --no-default-features --features wasm. The default `cli` feature pulls
#     clap and env_logger, which are dead weight in a wasm bundle.
#
# The wasm-pack call goes through dev-build.sh --exec so it gets the same
# Cargo.lock protection as any other cargo invocation in this repo.
#
# Usage:
#   ./wasm-build.sh                 # release bundler build -> pkg/
#   ./wasm-build.sh --target nodejs # any wasm-pack build flags
#   ./wasm-build.sh --dev           # unoptimized, much faster
#
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v wasm-pack >/dev/null 2>&1; then
    echo "wasm-build: wasm-pack not found (cargo install wasm-pack)" >&2
    exit 1
fi
if ! rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
    echo "wasm-build: rustup target add wasm32-unknown-unknown" >&2
    exit 1
fi

# Callers may pick their own profile and target; supply defaults only if they
# didn't. (Built by rewriting "$@" rather than an array, so that no arguments
# stays no arguments -- "${arr[@]}" on an empty array misbehaves in bash 3.2.)
profile_set=""
target_set=""
for arg in "$@"; do
    case $arg in
        --dev | --profiling | --release) profile_set=1 ;;
        --target | --target=*) target_set=1 ;;
    esac
done
[[ -n $profile_set ]] || set -- --release "$@"
[[ -n $target_set ]] || set -- "$@" --target bundler

export RUSTFLAGS="${RUSTFLAGS:-} --cfg getrandom_backend=\"wasm_js\""

exec ./dev-build.sh --exec wasm-pack build "$@" \
    -- --no-default-features --features wasm
