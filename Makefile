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

## check for unused dependencies using `cargo shear`
.PHONY: check-unused-deps
check-unused-deps: .installed-cargo-extensions.checkpoint
	cargo +nightly shear --expand

.PHONY: fix 
## Tries to auto-fix hygiene issues reported by `make check`. 
## Fixes will not be applied if there are uncommitted changes: to always apply fixes, use `make fix-dirty`.
fix: 
	cargo fmt --all
	cargo clippy -q --fix

.PHONY: fix-dirty
## fix, but applies fixes even when there are uncommitted changes.
fix-dirty:
	cargo fmt --all 
	cargo clippy -q --fix --allow-dirty --allow-staged

## install cargo extensions used in this Makefile (cargo-shear)
.PHONY: install-cargo-extensions
install-cargo-extensions: .installed-cargo-extensions.checkpoint

.installed-cargo-extensions.checkpoint: Makefile
	cargo install cargo-shear
	touch .installed-cargo-extensions.checkpoint


.PHONY: help
## Shows this help text
help: show-help

.DEFAULT_GOAL : help
