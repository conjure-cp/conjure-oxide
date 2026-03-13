# Make this Makefile auto-documenting
include tools/show-help-minified.make

# Extra flags to be passed to `cargo check` (default: -q).
EXTRA_CARGO_CHECK_FLAGS ?= -q
# Use Cargo.lock to ensure local builds match CI dependency versions.
# Override with `CARGO_LOCKED=` if you explicitly want to update the lockfile.
CARGO_LOCKED ?= --locked

.PHONY: check
## Runs all hygiene checks. These are the same checks that occur in CI for PRs.
check:
	RUSTFLAGS="-D warnings" cargo check $(EXTRA_CARGO_CHECK_FLAGS) $(CARGO_LOCKED) --workspace --all-targets
	cargo clippy $(EXTRA_CARGO_CHECK_FLAGS) $(CARGO_LOCKED) -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
	cargo fmt --check

.PHONY: check-unused-deps
## Check for unused dependencies using `cargo shear`
check-unused-deps: .installed-cargo-extensions.checkpoint
	cargo +nightly shear --expand

.PHONY: build
## Builds the conjure-oxide executable
build:
	cargo build $(CARGO_LOCKED) --bin conjure-oxide --release

.PHONY: install
## Installs conjure-oxide to ~/.cargo/bin
install: build
	mkdir -p $$HOME/.cargo/bin
	install -m 755 target/release/conjure-oxide $$HOME/.cargo/bin/conjure-oxide

.PHONY: test
## Runs all tests
test: install
	PATH="$$HOME/.cargo/bin:$$PATH" cargo test $(CARGO_LOCKED) --workspace

.PHONY: test-coverage
## Runs all tests and produces a coverage report
test-coverage:
	./tools/coverage.sh

.PHONY: test-accept
## Runs all tests in accept mode, then one more time in normal mode
test-accept: install
	PATH="$$HOME/.cargo/bin:$$PATH" ACCEPT=true cargo test $(CARGO_LOCKED) --workspace
	PATH="$$HOME/.cargo/bin:$$PATH" cargo test $(CARGO_LOCKED) --workspace

.PHONY: fix
## Tries to auto-fix hygiene issues reported by `make check`. 
## Fixes will not be applied if there are uncommitted changes: to always apply fixes, use `make fix-dirty`.
fix:
	cargo fmt --all
	cargo fix $(CARGO_LOCKED)
	cargo clippy -q $(CARGO_LOCKED) --fix

.PHONY: fix-dirty
## Tries to auto-fix hygiene issues reported by `make check`. 
## Applies fixes even when there are uncommitted changes.
fix-dirty:
	cargo fmt --all
	cargo fix $(CARGO_LOCKED) --allow-dirty --allow-staged
	cargo clippy -q $(CARGO_LOCKED) --fix --allow-dirty --allow-staged

# install cargo extensions used in this Makefile (cargo-shear)
.PHONY: install-cargo-extensions
install-cargo-extensions: .installed-cargo-extensions.checkpoint

.installed-cargo-extensions.checkpoint: Makefile
	cargo install cargo-shear
	touch .installed-cargo-extensions.checkpoint

test-clean:
	cd tests-integration/tests/integration/; find -type f -path '**generated**' -delete
	cd tests-integration/tests/integration/; find -type f -path '**expected**' -delete
	cd tests-integration/tests/integration/; find -type f -path '**stats**' -delete

.PHONY: help
## Shows this help text
help: show-help

.DEFAULT_GOAL : help
