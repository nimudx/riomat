#!/usr/bin/env sh
# EXPERIMENTAL: compila el .exe de Windows desde Linux con cargo-xwin (descarga el SDK de Windows).
# Requisitos: rustup target add x86_64-pc-windows-msvc && cargo install cargo-xwin
set -eu
cd "$(dirname "$0")/.."
cargo xwin build -p riomat --release --target x86_64-pc-windows-msvc
echo "Listo: target/x86_64-pc-windows-msvc/release/riomat.exe"
