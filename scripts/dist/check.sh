#!/usr/bin/env bash
# Checks a server binary built by build.sh before it is packaged (spec 8.2):
#
# - a glibc binary may need no `GLIBC_` symbol version above its floor;
# - a macOS, Linux or Alpine binary keeps its symbol table, so backtraces
#   name functions, and carries no debug info, which ships separately;
# - `clippings probe` must print JSON whose protocolVersion is the one the
#   extension speaks (extension/src/protocol.ts).
#
# Linux binaries probe in a container of their own architecture: debian:10,
# whose glibc is exactly 2.28, for `linux-*`, and Alpine for `alpine-*`. Both
# images are pinned below by their multi-arch index digest, so the exact
# same manifest list resolves on every platform instead of a moving tag. On
# an x64 host the arm64 and armv7 containers run under qemu, so the host
# needs binfmt handlers (docker/setup-qemu-action). macOS and Windows
# binaries run directly; pass --no-probe where the host cannot run them.
#
# Usage: scripts/dist/check.sh <vscode-target> <binary> [--no-probe]
set -euo pipefail

need() { command -v "$1" >/dev/null || { echo "check.sh: $1 not found" >&2; exit 1; }; }

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
# shellcheck source=target.sh
source "$here/target.sh"

target=${1:?usage: check.sh <vscode-target> <binary> [--no-probe]}
binary=${2:?usage: check.sh <vscode-target> <binary> [--no-probe]}
probe=${3:-}
resolve_target "$target"
[[ -f $binary ]] || { echo "$binary: not found" >&2; exit 1; }

if [[ -n $glibc ]]; then
  need readelf
  highest=$(readelf -V --wide "$binary" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | sort -uV | tail -n 1)
  if [[ -z $highest ]]; then
    echo "$target: no GLIBC_ symbol versions found; is this a glibc binary?" >&2
    exit 1
  fi
  if [[ $(printf '%s\n%s\n' "$highest" "$glibc" | sort -V | tail -n 1) != "$glibc" ]]; then
    echo "$target: needs GLIBC_$highest, above the $glibc floor" >&2
    exit 1
  fi
  echo "$target: highest glibc symbol version $highest (floor $glibc)"
fi

case "$target" in
  linux-* | alpine-*)
    need readelf
    sections=$(readelf -S --wide "$binary")
    symbols=$(readelf -s --wide "$binary")
    ;;
  darwin-*)
    need nm
    sections=""
    symbols=$(nm "$binary")
    ;;
  *)
    sections="" symbols=clippings4main
    ;;
esac
if [[ $symbols != *clippings4main* ]]; then
  echo "$target: no symbol for clippings::main; backtraces would not name functions" >&2
  exit 1
fi
if [[ $sections == *" .debug_info "* ]]; then
  echo "$target: the binary still carries debug info" >&2
  exit 1
fi
[[ $target == win32-* ]] || echo "$target: symbol table kept, no debug info"

if [[ $probe == --no-probe ]]; then
  echo "$target: probe skipped"
  exit 0
fi

need jq

case "$target" in
  linux-* | alpine-*)
    need docker
    case "$target" in
      *-x64) platform=linux/amd64 ;;
      *-arm64) platform=linux/arm64 ;;
      *-armhf) platform=linux/arm/v7 ;;
    esac
    case "$target" in
      # debian:10 as of 2026-09-27; index covers linux/amd64, linux/arm/v7,
      # linux/arm64/v8 and linux/386 (glibc 2.28, matching the floor above).
      linux-*) image=debian@sha256:58ce6f1271ae1c8a2006ff7d3e54e9874d839f573d8009c20154ad0f2fb0a225 ;;
      # alpine:3.22 as of 2026-09-27; index covers linux/amd64, linux/arm/v6,
      # linux/arm/v7, linux/arm64/v8, linux/386, linux/ppc64le, linux/riscv64
      # and linux/s390x.
      alpine-*) image=alpine@sha256:5291449c3df73caf6ed85e649dec1b9e818b39a5d8c871e97afc13e9cd5e8fa8 ;;
    esac
    dir=$(cd "$(dirname "$binary")" && pwd)
    json=$(docker run --rm --platform "$platform" -v "$dir:/probe:ro" "$image" "/probe/$(basename "$binary")" probe)
    ;;
  *)
    json=$("$binary" probe)
    ;;
esac

expected=$(sed -n 's/^export const PROTOCOL_VERSION = \([0-9][0-9]*\);$/\1/p' "$root/extension/src/protocol.ts")
[[ -n $expected ]] || { echo "PROTOCOL_VERSION not found in extension/src/protocol.ts" >&2; exit 1; }
echo "$target: probe: $json"
if ! jq -e --argjson v "$expected" '.protocolVersion == $v' <<<"$json" >/dev/null; then
  echo "$target: expected protocolVersion $expected" >&2
  exit 1
fi
