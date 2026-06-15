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
DEV_CONTAINER_IMAGE ?= conjure-oxide-dev
DEV_CONTAINER_FILE ?= Dockerfile.dev
CARGO_TEST_WORKSPACE = cargo test $(CARGO_LOCKED) $(CARGO_FEATURES) --workspace
# Golden files follow the test-suite convention of `.expected` or `-expected-` in the file name.
# This intentionally ignores config.toml, including expected-time-only changes.
RUN_NON_ACCEPTING_TESTS_IF_GOLDEN_FILES_CHANGED = if test -n "$$(git status --porcelain -- ':(glob)**/*.expected*' ':(glob)**/*-expected-*')"; then echo "Golden files changed; running tests without ACCEPT"; PATH="$$HOME/.cargo/bin:$$PATH" $(CARGO_TEST_WORKSPACE); else echo "No golden files changed; skipping non-accepting test run"; fi

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
	@mkdir -p $$HOME/.cargo/bin
	@install -m 755 $(CARGO_TARGET_DIR)/release/conjure-oxide $$HOME/.cargo/bin/conjure-oxide
	@install -m 755 $(CARGO_TARGET_DIR)/debug/conjure-oxide $$HOME/.cargo/bin/conjure-oxide-debug

.PHONY: test
## Runs all tests
test: submodules install
	PATH="$$HOME/.cargo/bin:$$PATH" $(CARGO_TEST_WORKSPACE)

.PHONY: test-coverage
## Runs all tests and produces a coverage report
test-coverage:
	./tools/coverage.sh

.PHONY: test-accept
## Runs all tests in accept mode, then in normal mode if golden files changed
test-accept: install
	PATH="$$HOME/.cargo/bin:$$PATH" ACCEPT=true $(CARGO_TEST_WORKSPACE)
	@$(RUN_NON_ACCEPTING_TESTS_IF_GOLDEN_FILES_CHANGED)

.PHONY: test-accept-with-slower-times
## Runs all tests in accept mode, only increases expected run times, then in normal mode if golden files changed
test-accept-with-slower-times: install
	PATH="$$HOME/.cargo/bin:$$PATH" ACCEPT=with-slower-times $(CARGO_TEST_WORKSPACE)
	@$(RUN_NON_ACCEPTING_TESTS_IF_GOLDEN_FILES_CHANGED)

.PHONY: test-accept-with-exact-times
## Runs all tests in accept mode, updates expected run times exactly, then in normal mode if golden files changed
test-accept-with-exact-times: install
	PATH="$$HOME/.cargo/bin:$$PATH" ACCEPT=with-exact-times $(CARGO_TEST_WORKSPACE)
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


.PHONY: build-container
## Builds the developer container image (Dockerfile.dev)
build-container:
	podman build -f $(DEV_CONTAINER_FILE) -t $(DEV_CONTAINER_IMAGE) .

.PHONY: run-in-container
## Runs a command in the developer container (usage: make run-in-container CMD="make build")
run-in-container:
	@test -n "$(CMD)"
	@podman run --rm -it \
	  --userns=keep-id \
	  -e HOME=/tmp \
	  -e CARGO_HOME=/tmp/cargo \
	  -v "$$PWD:/work:Z" \
	  -w /work \
	  $(DEV_CONTAINER_IMAGE) \
	  bash -lc 'mkdir -p "$$CARGO_HOME" && exec bash -lc "$(CMD)"'

# install cargo extensions used in this Makefile (cargo-shear)
.PHONY: install-cargo-extensions
install-cargo-extensions: .installed-cargo-extensions.checkpoint

.installed-cargo-extensions.checkpoint: Makefile
	cargo install cargo-shear
	touch .installed-cargo-extensions.checkpoint

test-clean:
	cd test-suite/tests/integration/; find -type f -path '**generated**' -delete
	cd test-suite/tests/integration/; find -type f -path '**expected**' -delete
	cd test-suite/tests/integration/; find -type f -path '**stats**' -delete

.PHONY: help
## Shows this help text
help: show-help

.DEFAULT_GOAL : help
