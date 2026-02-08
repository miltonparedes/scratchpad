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
    cargo test

# Run a single test by name
test-one name:
    cargo test -p scratchpad -- {{name}}

# Lint and format check
check:
    cargo fmt -- --check
    cargo clippy -- -D warnings

# Auto-fix formatting
fmt:
    cargo fmt

# Install sp binary from local source (dev build)
install:
    cargo install --path scratchpad

# Install sp-server binary from local source
install-server:
    cargo install --path server

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
