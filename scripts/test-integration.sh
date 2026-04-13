#!/usr/bin/env bash
set -euo pipefail

export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

cargo test -p infrastructure --test repositories -- --nocapture
cargo test -p api --test api_flows -- --test-threads=1 --nocapture
