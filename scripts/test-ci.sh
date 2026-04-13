#!/usr/bin/env bash
set -euo pipefail

export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

cargo fmt --all -- --check
./scripts/test-unit.sh
./scripts/test-integration.sh
./scripts/test-contracts.sh
./scripts/test-frontend.sh
cargo test --workspace --lib --bins
