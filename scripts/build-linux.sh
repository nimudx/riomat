#!/usr/bin/env sh
# Compila y prueba Riomat en Linux. Uso: scripts/build-linux.sh [run]
set -eu
cd "$(dirname "$0")/.."
cargo test --workspace --release
cargo build -p riomat --release
echo "Listo: target/release/riomat"
[ "${1:-}" = "run" ] && exec target/release/riomat
