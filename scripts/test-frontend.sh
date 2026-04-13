#!/usr/bin/env bash
set -euo pipefail

if [ ! -f "package.json" ] && [ ! -d "apps/web" ] && [ ! -d "apps/frontend" ]; then
  echo "Skipping frontend component tests: no frontend application exists in this workspace yet."
  exit 0
fi

echo "Frontend test harness is not wired yet. Add the frontend workspace and replace this stub."
