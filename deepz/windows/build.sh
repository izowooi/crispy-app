#!/bin/bash
# macOS 에서 Windows 용 Deepz.exe 를 만든다. (필요: rustup target add x86_64-pc-windows-gnu, brew install mingw-w64)
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target x86_64-pc-windows-gnu
mkdir -p ../dist
cp target/x86_64-pc-windows-gnu/release/deepz.exe ../dist/Deepz.exe
ls -lh ../dist/Deepz.exe
