#!/usr/bin/env bash
set -euo pipefail

if [ ! -d "contracts" ] && [ ! -d "soroban" ]; then
  echo "Skipping Soroban contract tests: no contracts/ or soroban/ directory exists yet."
  exit 0
fi

echo "Soroban contract test harness is not wired yet. Add contract workspace and replace this stub."
