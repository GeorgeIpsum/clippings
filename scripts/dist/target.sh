# shellcheck shell=bash
# The variables set here are read by the scripts that source this file.
# shellcheck disable=SC2034
# Sourced by the other scripts in this directory. Maps a VS Code target
# (spec 8.1) to its Rust triple and build facts:
#
#   triple   the Rust target triple
#   glibc    the glibc floor for `-gnu` targets, empty otherwise
#   exe      the binary's file name
#
# Usage: resolve_target <vscode-target>

resolve_target() {
  glibc=""
  exe=clippings
  case "$1" in
    win32-x64) triple=x86_64-pc-windows-msvc exe=clippings.exe ;;
    win32-arm64) triple=aarch64-pc-windows-msvc exe=clippings.exe ;;
    linux-x64) triple=x86_64-unknown-linux-gnu glibc=2.28 ;;
    linux-arm64) triple=aarch64-unknown-linux-gnu glibc=2.28 ;;
    linux-armhf) triple=armv7-unknown-linux-gnueabihf glibc=2.28 ;;
    alpine-x64) triple=x86_64-unknown-linux-musl ;;
    alpine-arm64) triple=aarch64-unknown-linux-musl ;;
    darwin-x64) triple=x86_64-apple-darwin ;;
    darwin-arm64) triple=aarch64-apple-darwin ;;
    *)
      echo "unknown VS Code target: $1" >&2
      return 1
      ;;
  esac
}
