# Default: list recipes
default:
    @just --list

# Build entire workspace
build:
    cargo build

# Build in release mode
release:
    cargo build --release

# Run all tests
test:
    cargo test --workspace --all-targets

# Run a single test by name
test-one name:
    cargo test -p scratchpad -- {{name}}

# Lint and format check
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

# Run the same local checks as CI
ci: check test

# Auto-fix formatting
fmt:
    cargo fmt --all

# Install sp binary from local source (dev build)
install:
    cargo install --locked --path scratchpad

# Install sp-server binary from local source
install-server:
    cargo install --locked --path server

# Launch TUI (dev)
run *args:
    cargo run -p scratchpad -- {{args}}

# Start sync server (dev)
serve:
    cargo run -p scratchpad-server

# Bump version in scratchpad crate and update lockfile
bump version:
    sed -i '' 's/^version = ".*"/version = "{{version}}"/' scratchpad/Cargo.toml
    cargo check -p scratchpad
    @echo "Bumped to {{version}} — commit and tag with: git tag v{{version}}"

# Clean build artifacts
clean:
    cargo clean
