#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "Usage: $0 HORIZON_CHECKOUT [check|test|build]" >&2
    exit 2
fi

source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
game_dir=$(cd -- "$1" && pwd)
mode=${2:-build}
case "$mode" in
    check|test|build) ;;
    *) echo "Unknown mode: $mode" >&2; exit 2 ;;
esac

pin=$(sed -nE 's/^sdk = "([[:xdigit:]]{40})"$/\1/p' "$source_dir/Cargo.toml")
if [[ -z "$pin" ]] || ! git -C "$game_dir" merge-base --is-ancestor "$pin" HEAD; then
    echo "The Horizon checkout must contain API baseline $pin." >&2
    exit 1
fi

mkdir -p "$game_dir/target"
work=$(mktemp -d "$game_dir/target/slimefun-mod.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT

cp "$source_dir/horizon-mod.toml" "$work/"
cp -R "$source_dir/src" "$work/"
if [[ "$mode" == test ]]; then
    mkdir -p "$work/tools/paper-storage-export"
    cp -R "$source_dir/tools/paper-storage-export/fixtures" "$work/tools/paper-storage-export/"
fi
if [[ -f "$source_dir/Cargo.lock" ]]; then
    cp "$source_dir/Cargo.lock" "$work/"
fi

# Building inside the checkout uses its Mod API source without downloading an SDK.
sed 's#\.horizon/sdk/crates/horizon-mod-api#../../crates/horizon-mod-api#' \
    "$source_dir/Cargo.toml" > "$work/Cargo.toml"

case "$mode" in
    check|test)
        CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$game_dir/target/slimefun-check"} \
            cargo "$mode" --locked --manifest-path "$work/Cargo.toml"
        ;;
    build)
        tool_dir=${CARGO_TARGET_DIR:-"$game_dir/target"}
        mkdir -p "$tool_dir"
        tool_dir=$(cd -- "$tool_dir" && pwd)
        CARGO_TARGET_DIR="$tool_dir" cargo build \
            --manifest-path "$game_dir/Cargo.toml" --locked -p cargo-horizon-mod
        (
            cd -- "$work"
            "$tool_dir/debug/cargo-horizon-mod" build --wasm
        )
        install -d "$source_dir/target/horizon-mod"
        install -m 0644 "$work/target/horizon-mod/slimefun.hmod" \
            "$source_dir/target/horizon-mod/slimefun.hmod"
        ;;
esac
