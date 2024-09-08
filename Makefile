.PHONY: mainnet devnet helios_mainnet

mainnet:
	cargo run -- --mainnet --retry $(retry)

devnet:
	cargo run -- --devnet --retry $(retry)

helios_mainnet:
	cargo run -- --helios-mainnet --retry $(retry)

retry=$(filter-out $@,$(MAKECMDGOALS))

%:
	@:
