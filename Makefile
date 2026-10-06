.PHONY: help build test clean format lint install-deps build-wasm optimize

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-20s %s\n", $$1, $$2}'

install-deps: ## Install required dependencies
	@echo "Installing Rust toolchain..."
	rustup target add wasm32-unknown-unknown
	@echo "Installing Soroban CLI..."
	cargo install --locked soroban-cli --features opt

build: ## Build all contracts in debug mode
	cargo build

build-release: ## Build all contracts in release mode
	cargo build --release

build-wasm: ## Build WASM binaries for all contracts
	@echo "Building asset_factory..."
	cargo build --package asset_factory --target wasm32-unknown-unknown --release
	@echo "Building rwa_token..."
	cargo build --package rwa_token --target wasm32-unknown-unknown --release
	@echo "Building marketplace_escrow..."
	cargo build --package marketplace_escrow --target wasm32-unknown-unknown --release

optimize: build-wasm ## Optimize WASM binaries
	@echo "Optimizing asset_factory..."
	soroban contract optimize --wasm target/wasm32-unknown-unknown/release/asset_factory.wasm
	@echo "Optimizing rwa_token..."
	soroban contract optimize --wasm target/wasm32-unknown-unknown/release/rwa_token.wasm
	@echo "Optimizing marketplace_escrow..."
	soroban contract optimize --wasm target/wasm32-unknown-unknown/release/marketplace_escrow.wasm

test: ## Run all tests
	cargo test

test-verbose: ## Run tests with verbose output
	cargo test -- --nocapture

test-asset-factory: ## Run asset_factory tests
	cargo test --package asset_factory

test-rwa-token: ## Run rwa_token tests
	cargo test --package rwa_token

test-marketplace: ## Run marketplace_escrow tests
	cargo test --package marketplace_escrow

format: ## Format code using rustfmt
	cargo fmt

format-check: ## Check code formatting
	cargo fmt -- --check

lint: ## Run clippy linter
	cargo clippy -- -D warnings

clean: ## Clean build artifacts
	cargo clean
	rm -f target/wasm32-unknown-unknown/release/*.wasm
	rm -f target/wasm32-unknown-unknown/release/*.optimized.wasm

check: ## Run cargo check
	cargo check

watch: ## Watch for changes and run tests
	cargo watch -x test

watch-check: ## Watch for changes and run check
	cargo watch -x check

all: format lint test build-wasm optimize ## Run all checks and build

deploy-testnet: optimize ## Deploy contracts to Stellar testnet (requires STELLAR_SECRET_KEY env var)
	@if [ -z "$$STELLAR_SECRET_KEY" ]; then \
		echo "Error: STELLAR_SECRET_KEY environment variable not set"; \
		exit 1; \
	fi
	@echo "Deploying asset_factory..."
	soroban contract deploy \
		--wasm target/wasm32-unknown-unknown/release/asset_factory.optimized.wasm \
		--source $$STELLAR_SECRET_KEY \
		--network testnet
	@echo "Deploying rwa_token..."
	soroban contract deploy \
		--wasm target/wasm32-unknown-unknown/release/rwa_token.optimized.wasm \
		--source $$STELLAR_SECRET_KEY \
		--network testnet
	@echo "Deploying marketplace_escrow..."
	soroban contract deploy \
		--wasm target/wasm32-unknown-unknown/release/marketplace_escrow.optimized.wasm \
		--source $$STELLAR_SECRET_KEY \
		--network testnet
