#!/usr/bin/env bash
set -euo pipefail

export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

cargo test -p domain
cargo test -p application
