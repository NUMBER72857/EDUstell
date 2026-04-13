#!/usr/bin/env bash
set -euo pipefail

if [ -z "${DATABASE_URL:-}" ]; then
  echo "DATABASE_URL must be set before seeding local data."
  exit 1
fi

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f scripts/seed_local.sql
