#!/bin/bash
# Builds the standalone VRPTW solver for Apple Silicon (run on a Mac with Rust installed).
# Result: dist/hgs_vrptw  (self-contained: no Rust, no dependencies needed to run it)
set -e
cd "$(dirname "$0")/.."
[ "$(uname -s)" = "Darwin" ] || { echo "Run this on the Mac (macOS), the binary must be built for macOS."; exit 1; }
cargo build --release
mkdir -p dist
cp target/release/hgs_advance dist/hgs_vrptw
strip dist/hgs_vrptw
codesign --force -s - dist/hgs_vrptw          # ad-hoc signature (required to run on Apple Silicon)
echo; file dist/hgs_vrptw; echo "Linked libraries:"; otool -L dist/hgs_vrptw | tail -n +2
echo; ls -lh dist/hgs_vrptw
echo; echo "Usage:  ./hgs_vrptw <instance.txt> [time_limit_seconds]"
