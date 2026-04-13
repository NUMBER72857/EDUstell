.PHONY: up down fmt check test test-unit test-integration test-contracts test-frontend migrate seed

up:
	docker compose up -d postgres

down:
	docker compose down

fmt:
	cargo fmt --all

check:
	cargo check --workspace

test:
	cargo test --workspace

test-unit:
	./scripts/test-unit.sh

test-integration:
	./scripts/test-integration.sh

test-contracts:
	./scripts/test-contracts.sh

test-frontend:
	./scripts/test-frontend.sh

migrate:
	sqlx migrate run

seed:
	./scripts/seed-local.sh
