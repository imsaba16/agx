# Contributing to Antigravity eXtension (`agx`)

Thank you for your interest in contributing to **Antigravity eXtension (`agx`)**! We welcome contributions of all kinds: bug reports, documentation enhancements, feature proposals, and code improvements.

---

## Code of Conduct

All contributors and maintainers are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md). Please read it before participating in discussions or submitting pull requests.

---

## Development Setup

### Prerequisites

- **Rust toolchain**: 1.74.0 or newer (via [rustup](https://rustup.rs/)).
- **Cargo**: Bundled with Rust.
- **Git**: For version control.

Verify your environment:
```bash
rustc --version
cargo --version
```

### Cloning the Repository

```bash
git clone https://github.com/imsaba16/agx.git
cd agx
```

### Building the Project

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

---

## Running Tests & Quality Checks

Before submitting a pull request, verify that all tests and linter checks pass locally.

### Unit & Integration Tests

```bash
cargo test --all-targets
```

### Linter (Clippy)

We enforce zero clippy warnings in the codebase:

```bash
cargo clippy --all-targets -- -D warnings
```

### Formatting

Format code according to Rust standard conventions:

```bash
cargo fmt --all --check
```

To auto-format files:
```bash
cargo fmt --all
```

---

## Development Guidelines

### 1. Code Architecture

- **`src/cli.rs`**: Definition of CLI arguments, commands, flags, and Clap parsers.
- **`src/bundle.rs`**: Core archive packaging, extraction, inspection, and migration logic.
- **`src/project.rs`**: Multi-conversation project workspace bundles (`.agproject`).
- **`src/power_user.rs`**: Streaming query engine (`diff`, `search`, `extract`, `cat`, `explore`).
- **`src/security.rs`**: Cryptographic integrity auditing, Zip-Slip path sanitization, and secret masking.
- **`src/sanitizer.rs`**: Token detection, key masking, and regex pattern matching.
- **`src/ui.rs`**: Terminal user interface, theme styling, spinners, and interactive dialogs.

### 2. Security First

`agx` is designed for enterprise environments. Any code changes must respect the security model:
- **Zero Path Traversal**: Never extract paths that contain `..`, leading slashes, or reference outside the target directory.
- **Decompression Bomb Protection**: Always inspect uncompressed archive size limits before extraction.
- **Secret Redaction**: Do not bypass secret redaction rules unless explicitly disabled by user flags.
- **Permission Sanitization**: Extract files with sanitized permissions (`0o644` for files, `0o755` for directories).

### 3. Commit Convention

We use the [Conventional Commits](https://www.conventionalcommits.org/) specification:

- `feat(scope)`: A new feature
- `fix(scope)`: A bug fix
- `docs(scope)`: Documentation changes only
- `refactor(scope)`: Code changes that neither fix a bug nor add a feature
- `perf(scope)`: Performance improvements
- `test(scope)`: Adding or correcting tests
- `chore(scope)`: Maintenance or tooling tasks

Example:
```
feat(power-user): add regex flag support to agx search
fix(security): prevent symlink traversal in project packing
docs(cli): add documentation for agx explore interactive viewer
```

---

## Submitting Pull Requests

1. **Fork & Branch**: Create a feature branch off `development`:
   ```bash
   git checkout -b feat/your-feature-name
   ```
2. **Implement**: Write clean, idiomatic, well-commented code with unit tests.
3. **Verify**: Ensure `cargo test` and `cargo clippy -- -D warnings` pass.
4. **Push & PR**: Open a pull request against the `development` branch with a clear description of the problem solved and test coverage.

---

## License

By contributing to Antigravity eXtension, you agree that your contributions will be licensed under the [Apache 2.0 License](LICENSE).
