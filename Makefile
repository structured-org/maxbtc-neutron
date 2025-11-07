.PHONY: schema test clippy build fmt compile check_contracts

schema:
	@find contracts/* -maxdepth 2 -type f -name Cargo.toml -execdir cargo schema \;
test:
	@cargo test

clippy:
	@rustup target add wasm32-unknown-unknown
	@cargo clippy --all --all-targets -- -D warnings
	@cargo clippy --lib --target wasm32-unknown-unknown -- -D warnings

fmt:
	@cargo fmt -- --check

doc:
	@cargo doc

compile:
	@docker run --rm -v "$(CURDIR)":/code \
		--mount type=volume,source="$(notdir $(CURDIR))_cache",target=/target \
		--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
		--platform linux/amd64 \
		cosmwasm/optimizer:0.17.0
	@sudo chown -R $(shell id -u):$(shell id -g) artifacts

compile_arm64:
	@docker run --rm -v "$(CURDIR)":/code \
		--mount type=volume,source="$(notdir $(CURDIR))_cache",target=/target \
		--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
		--platform linux/arm64 \
		cosmwasm/optimizer-arm64:0.17.0

build_arm64: schema clippy test fmt doc compile_arm64

build: schema clippy test fmt doc compile

build_ts_client: schema
	@cd ts-client && yarn && yarn generate
