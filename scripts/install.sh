#!/usr/bin/env sh
set -eu

cargo build --release -p sing_cli
echo "built target/release/sing"
