# Make this Makefile auto-documenting
include tools/show-help-minified.make

# Extra flags to be passed to `cargo check` (default: -q).
EXTRA_CARGO_CHECK_FLAGS ?= -q

.PHONY: check
## Runs all hygiene checks. These are the same checks that occur in CI for PRs.
check:
	RUSTFLAGS="-D warnings" cargo check $(EXTRA_CARGO_CHECK_FLAGS) --workspace
	RUSTFLAGS="-D warnings" cargo check $(EXTRA_CARGO_CHECK_FLAGS) --workspace --examples
	cargo clippy $(EXTRA_CARGO_CHECK_FLAGS) -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
	cargo fmt --check

.PHONY: check-unused-deps
## Check for unused dependencies using `cargo shear`
check-unused-deps: .installed-cargo-extensions.checkpoint
	cargo +nightly shear --expand

.PHONY: test
## Runs all tests
test:
	cargo build --bin conjure-oxide # we need to build first, so the conjure-oxide executable is available during testing as it is needed by the custom tests.
	cargo test --workspace

.PHONY: test-coverage
## Runs all tests and produces a coverage report
test-coverage:
	./tools/coverage.sh

.PHONY: test-accept
## Runs all tests in accept mode, then one more time in normal mode
test-accept:
	cargo build --bin conjure-oxide
	ACCEPT=true cargo test --workspace
	cargo test --workspace

.PHONY: fix
## Tries to auto-fix hygiene issues reported by `make check`. 
## Fixes will not be applied if there are uncommitted changes: to always apply fixes, use `make fix-dirty`.
fix:
	cargo fmt --all
	cargo fix
	cargo clippy -q --fix

.PHONY: fix-dirty
## Tries to auto-fix hygiene issues reported by `make check`. 
## Applies fixes even when there are uncommitted changes.
fix-dirty:
	cargo fmt --all
	cargo fix --allow-dirty --allow-staged
	cargo clippy -q --fix --allow-dirty --allow-staged

# install cargo extensions used in this Makefile (cargo-shear)
.PHONY: install-cargo-extensions
install-cargo-extensions: .installed-cargo-extensions.checkpoint

.installed-cargo-extensions.checkpoint: Makefile
	cargo install cargo-shear
	touch .installed-cargo-extensions.checkpoint

.PHONY: help
## Shows this help text
help: show-help

.DEFAULT_GOAL : help
