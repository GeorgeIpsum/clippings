#!/usr/bin/env bash
# Builds the shipped server for one VS Code target (spec 8.1, 8.2) with the
# `dist` profile, then moves its debug info into a separate symbols file:
#
#   <out>/<target>/clippings[.exe]              the binary, without debug info
#   <out>/symbols/clippings-<target>.dSYM.zip   macOS
#   <out>/symbols/clippings-<target>.pdb        Windows
#   <out>/symbols/clippings-<target>.debug      Linux and Alpine
#
# macOS and Windows targets build with cargo on their own runners. Linux and
# Alpine targets build with cargo-zigbuild, using the glibc-suffixed target
# for the 2.28 floor; set CLIPPINGS_BUILD_TOOL=cargo to build a Linux target
# with plain cargo instead, as the debian:10 fallback does. Linux symbols are
# split with llvm-objcopy, found in the rustup `llvm-tools` component or on
# PATH, or named by $OBJCOPY.
#
# macOS and Linux binaries keep their symbol table, so a panic's backtrace
# (the client sets RUST_BACKTRACE=1) names its functions; file and line
# numbers need the symbols file. MSVC binaries never carry a symbol table:
# Windows backtraces name functions only when the PDB is beside the binary.
#
# Cargo's output is found in $CARGO_TARGET_DIR when that is set.
#
# Usage: scripts/dist/build.sh <vscode-target> [out-dir]
#   out-dir defaults to dist; a relative one is relative to the current
#   directory, like any other path argument.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
# shellcheck source=target.sh
source "$here/target.sh"

target=${1:?usage: build.sh <vscode-target> [out-dir]}
out=${2:-dist}
resolve_target "$target"
mkdir -p "$out/$target" "$out/symbols"
out=$(cd "$out" && pwd)
# Cargo reads a relative CARGO_TARGET_DIR against the current directory.
target_dir=${CARGO_TARGET_DIR:-$root/target}
case "$target_dir" in
  /* | [A-Za-z]:*) ;;
  *) target_dir=$PWD/$target_dir ;;
esac
built=$target_dir/$triple/dist

cargo_args=(--locked --profile dist -p clippings --manifest-path "$root/Cargo.toml")

case "$target" in
  darwin-*)
    # rustc runs dsymutil on the linked binary, then strips its debug info.
    CARGO_PROFILE_DIST_SPLIT_DEBUGINFO=packed CARGO_PROFILE_DIST_STRIP=debuginfo \
      cargo build "${cargo_args[@]}" --target "$triple"
    cp "$built/$exe" "$out/$target/$exe"
    # target/<triple>/dist/clippings.dSYM is a symlink into deps/.
    staging=$(mktemp -d)
    cp -RL "$built/$exe.dSYM" "$staging/$exe.dSYM"
    rm -f "$out/symbols/clippings-$target.dSYM.zip"
    (cd "$staging" && ditto -c -k --norsrc --noextattr --noacl --keepParent "$exe.dSYM" "$out/symbols/clippings-$target.dSYM.zip")
    rm -rf "$staging"
    ;;
  win32-*)
    # MSVC always writes debug info to a PDB beside the binary, so the
    # binary carries none; `strip` would suppress the PDB instead.
    cargo build "${cargo_args[@]}" --target "$triple"
    cp "$built/$exe" "$out/$target/$exe"
    cp "$built/clippings.pdb" "$out/symbols/clippings-$target.pdb"
    ;;
  linux-* | alpine-*)
    if [[ ${CLIPPINGS_BUILD_TOOL:-zigbuild} == cargo ]]; then
      cargo build "${cargo_args[@]}" --target "$triple"
    else
      cargo zigbuild "${cargo_args[@]}" --target "$triple${glibc:+.$glibc}"
    fi
    if [[ -z ${OBJCOPY:-} ]]; then
      host=$(rustc -vV | sed -n 's/^host: //p')
      OBJCOPY=$(rustc --print sysroot)/lib/rustlib/$host/bin/llvm-objcopy
      [[ -x $OBJCOPY ]] || OBJCOPY=llvm-objcopy
    fi
    debug=$out/symbols/clippings-$target.debug
    cp "$built/$exe" "$out/$target/$exe"
    "$OBJCOPY" --only-keep-debug "$out/$target/$exe" "$debug"
    # The debug link names the symbols file by its base name, so gdb and
    # lldb find it when it sits beside the binary or in a debug directory.
    (cd "$out/symbols" && "$OBJCOPY" --strip-debug --add-gnu-debuglink="clippings-$target.debug" "$out/$target/$exe")
    ;;
esac

chmod 755 "$out/$target/$exe"
ls -l "$out/$target/$exe" "$out/symbols/clippings-$target".*
