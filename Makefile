# Make this Makefile auto-documenting
include tools/show-help-minified.make

# Extra flags to be passed to `cargo check` (default: -q).
EXTRA_CARGO_CHECK_FLAGS ?= -q
# Use Cargo.lock to ensure local builds match CI dependency versions.
# Override with `CARGO_LOCKED=` if you explicitly want to update the lockfile.
CARGO_LOCKED ?= --locked
# Extra feature flags to be passed to Cargo (e.g. --features z3-bundled).
CARGO_FEATURES ?=
CARGO_TARGET_DIR ?= target
CARGO_BIN_DIR ?= $(HOME)/.cargo/bin
DEV_CONTAINER_IMAGE ?= conjure-oxide-dev
DEV_CONTAINER_FILE ?= Dockerfile.dev
CARGO_TEST_WORKSPACE = cargo nextest run --release $(CARGO_LOCKED) $(CARGO_FEATURES) --workspace
CARGO_DOC_TEST_WORKSPACE = cargo test --release $(CARGO_LOCKED) $(CARGO_FEATURES) --workspace --doc
export PATH := $(CARGO_BIN_DIR):$(PATH)
# Golden files follow the test-suite convention of `.expected` or `-expected-` in the file name.
# This intentionally ignores config.toml, including expected-time-only changes.
RUN_NON_ACCEPTING_TESTS_IF_GOLDEN_FILES_CHANGED = if test -n "$$(git status --porcelain -- ':(glob)**/*.expected*' ':(glob)**/*-expected-*')"; then echo "Golden files changed; running tests without ACCEPT"; $(CARGO_DOC_TEST_WORKSPACE); $(CARGO_TEST_WORKSPACE); else echo "No golden files changed; skipping non-accepting test run"; fi

.PHONY: submodules
## Initialises git submodules needed for builds
submodules:
	git submodule update --init --recursive -- crates/minion-sys/vendor

.PHONY: check
## Runs all hygiene checks. These are the same checks that occur in CI for PRs.
check: submodules
	RUSTFLAGS="-D warnings" cargo check $(EXTRA_CARGO_CHECK_FLAGS) $(CARGO_LOCKED) $(CARGO_FEATURES) --workspace --all-targets
	cargo clippy $(EXTRA_CARGO_CHECK_FLAGS) $(CARGO_LOCKED) $(CARGO_FEATURES) -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
	cargo fmt --check

.PHONY: check-unused-deps
## Check for unused dependencies using `cargo shear`
check-unused-deps: .installed-cargo-extensions.checkpoint
	cargo +nightly shear --expand

.PHONY: build-release
## Builds the release conjure-oxide executable
build-release: submodules
	cargo build $(CARGO_LOCKED) $(CARGO_FEATURES) --bin conjure-oxide --release

.PHONY: build-debug
## Builds the debug conjure-oxide executable
build-debug: submodules
	cargo build $(CARGO_LOCKED) $(CARGO_FEATURES) --bin conjure-oxide

.PHONY: build
## Builds both release and debug conjure-oxide executables
build: build-release build-debug

.PHONY: install
## Installs release conjure-oxide and debug conjure-oxide-debug to ~/.cargo/bin
install: build
	@mkdir -p $(CARGO_BIN_DIR)
	@install -m 755 $(CARGO_TARGET_DIR)/release/conjure-oxide $(CARGO_BIN_DIR)/conjure-oxide
	@install -m 755 $(CARGO_TARGET_DIR)/debug/conjure-oxide $(CARGO_BIN_DIR)/conjure-oxide-debug

.PHONY: test
## Runs all tests
test: submodules install .installed-cargo-nextest.checkpoint
	$(CARGO_DOC_TEST_WORKSPACE)
	$(CARGO_TEST_WORKSPACE)

.PHONY: test-coverage
## Runs all tests and produces a coverage report
test-coverage:
	./tools/coverage.sh

.PHONY: test-accept
## Runs all tests in accept mode, then in normal mode if golden files changed
test-accept: install .installed-cargo-nextest.checkpoint
	ACCEPT=true $(CARGO_DOC_TEST_WORKSPACE)
	ACCEPT=true $(CARGO_TEST_WORKSPACE)
	@$(RUN_NON_ACCEPTING_TESTS_IF_GOLDEN_FILES_CHANGED)

.PHONY: test-accept-with-slower-times
## Runs all tests in accept mode, only increases expected run times, then in normal mode if golden files changed
test-accept-with-slower-times: install .installed-cargo-nextest.checkpoint
	ACCEPT=with-slower-times $(CARGO_DOC_TEST_WORKSPACE)
	ACCEPT=with-slower-times $(CARGO_TEST_WORKSPACE)
	@$(RUN_NON_ACCEPTING_TESTS_IF_GOLDEN_FILES_CHANGED)

.PHONY: test-accept-with-exact-times
## Runs all tests in accept mode, updates expected run times exactly, then in normal mode if golden files changed
test-accept-with-exact-times: install .installed-cargo-nextest.checkpoint
	ACCEPT=with-exact-times $(CARGO_DOC_TEST_WORKSPACE)
	ACCEPT=with-exact-times $(CARGO_TEST_WORKSPACE)
	@$(RUN_NON_ACCEPTING_TESTS_IF_GOLDEN_FILES_CHANGED)

.PHONY: fix
## Tries to auto-fix hygiene issues reported by `make check`. 
## Fixes will not be applied if there are uncommitted changes: to always apply fixes, use `make fix-dirty`.
fix:
	cargo fmt --all
	cargo fix $(CARGO_LOCKED) $(CARGO_FEATURES)
	cargo clippy -q $(CARGO_LOCKED) $(CARGO_FEATURES) --fix

.PHONY: fix-dirty
## Tries to auto-fix hygiene issues reported by `make check`. 
## Applies fixes even when there are uncommitted changes.
fix-dirty:
	cargo fmt --all
	cargo fix $(CARGO_LOCKED) $(CARGO_FEATURES) --allow-dirty --allow-staged
	cargo clippy -q $(CARGO_LOCKED) $(CARGO_FEATURES) --fix --allow-dirty --allow-staged

# install cargo extensions used in this Makefile (cargo-shear)
.PHONY: install-cargo-extensions
install-cargo-extensions: .installed-cargo-extensions.checkpoint

.installed-cargo-extensions.checkpoint: Makefile
	cargo install cargo-shear
	touch .installed-cargo-extensions.checkpoint

.installed-cargo-nextest.checkpoint: Makefile
	@if ! command -v cargo-nextest >/dev/null 2>&1; then cargo install cargo-nextest --locked; fi
	touch .installed-cargo-nextest.checkpoint

test-clean:
	cd test-suite/tests/integration/; find -type f -path '**generated**' -delete
	cd test-suite/tests/integration/; find -type f -path '**expected**' -delete
	cd test-suite/tests/integration/; find -type f -path '**stats**' -delete

.PHONY: help
## Shows this help text
help: show-help

.DEFAULT_GOAL : help
