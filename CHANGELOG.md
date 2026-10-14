# Changelog

All notable changes to the **Antigravity eXtension (`agx`)** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0-rc.3] - 2026-10-14

### Added
- **Multi-Platform Release Matrix (Linux ARM64 & Windows ARM64)**:
  - Added native Linux ARM64 (`aarch64-unknown-linux-gnu`) via `ubuntu-24.04-arm` runners.
  - Added native Windows ARM64 (`aarch64-pc-windows-msvc`) cross-compilation pipeline.
  - Automated dual-architecture Scoop bucket deployment (`64bit` and `arm64`) in `bucket/agx.json`.
- **Interactive Project & Workspace Selection in TUI**:
  - Running `agx` outside of a project root (e.g., from `Downloads`) now dynamically inspects the database and presents an interactive project picker rather than failing or cancelling silently.
  - Surfaced all export error diagnostics and added confirmation prompts before returning to the dashboard.

### Fixed
- **Windows IDE Launch & Process Lifecycle**:
  - Implemented automatic resolution of the Antigravity installation path across `%LOCALAPPDATA%\Programs\antigravity`, `%ProgramFiles%`, and system PATH.
  - Replaced `cmd.exe /c start` with direct detached process invocation (`DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`), eliminating the "Windows cannot find 'Antigravity.exe'" dialog.
  - Added case-insensitive process inspection and fallback force termination (`taskkill /F`) if graceful termination times out.
- **Homebrew Formula Architecture Compatibility**:
  - Modernized `Formula/agx.rb` to use `on_intel` and `on_arm` blocks for macOS and Linux, resolving `formula requires at least a URL` errors on AMD and ARM64 Linux systems.
- **Windows Path Normalization in Database Queries**:
  - Normalized drive letters (`c:` vs `C:`), slashes (`\` vs `/`), and URL encoding (`%3a`) when querying conversation workspace associations.
- **Linter & Formatting Cleanliness**:
  - Fixed Windows-specific `clippy::needless_borrows_for_generic_args` warnings and ensured zero compiler/clippy warnings under strict `-D warnings`.

---

## [1.0.0-rc.2] - 2026-10-14

### Added
- **Protobuf Summaries Store Synchronization**:
  - Implemented zero-dependency Protobuf wire encoding/decoding engine (`summaries_pb.rs`) for `agyhub_summaries_proto.pb` and `jetbox_summaries_proto.pb`.
  - Ensures imported projects and conversation histories render in the Antigravity sidebar, project drawers, and prompt snippet drawers.
  - Automatic collision-free UUID remapping for trajectory metadata and fallback synthesis of valid `CascadeTrajectorySummary` blobs.
- **IDE Lifecycle & Automatic Restart**:
  - Added `agx restart` command with cross-platform support (macOS AppleScript / open, Linux SIGTERM / respawn, Windows taskkill / start).
  - Added `--restart` (alias: `--auto-restart`) flag to `agx import` and `agx project import`.
  - Added interactive prompt offering to gracefully restart Antigravity upon completing an archive import.
- **Prioritized Interactive Dashboard**:
  - Reorganized the interactive dashboard (`agx`) into a clean, priority-based 5-item hierarchy: `Import Archive...`, `Export...`, `Interactive TUI Explorer`, `Restart Antigravity`, `More Tools & Utilities...`, eliminating terminal overflow and visual clutter.

---

## [1.0.0-rc.1] - 2026-10-13

### Changed
- **Rebranding**: Renamed project to **Antigravity eXtension (`agx`)** across CLI banners, help text, documentation, and agent skill manifests.
- **Terminal UI**: Refactored CLI into a sleek, professional theme with minimal headers, subtle borders, high-contrast typography, and eliminated non-standard emojis in interactive menus.
- **License**: Adopted the Apache 2.0 open-source license.

### Added
- **Power-User Command Engine**:
  - `agx diff <bundle1> <bundle2>`: Compare conversation manifests, artifacts, step counts, and content diffs across sessions or workspaces.
  - `agx search <archive> <query>`: High-speed query engine searching conversation transcripts, thoughts, and artifacts without full extraction. Supports `--case-insensitive`, `--regex`, and JSON output.
  - `agx extract <archive>`: Targeted extraction of single files or directory subtrees from bundles without unzipping the entire package. Supports `--stdout` piping.
  - `agx cat <archive> <path>`: Direct terminal output of any archived artifact or transcript with auto-detected syntax highlighting.
  - `agx explore <archive>`: Interactive TUI bundle browser with live preview and quick file export.
- **Project Workspaces (`.agproject`)**:
  - Full project workspace exports bundling multiple conversation sessions, brain memory directories, and local customizations (`.agents/skills`, `.agent/rules`, MCP configurations).
  - Selective conversation bundling and intelligent customization scanning.
- **Security & Hardening**:
  - Cryptographic verification via `agx verify` auditing SHA-256 hashes for all packaged files.
  - Path traversal safeguards preventing Zip-Slip attacks on malicious relative paths (`..`).
  - Decompression bomb threshold guards (maximum archive expansion limits).
  - Automated regex secret detection and masking for API keys, AWS credentials, and private keys.
  - Canonical permission sanitization (`0o644` files, `0o755` dirs).
- **Shell Autocompletion**:
  - Added `agx completion <shell>` generating native completion scripts for Bash, Zsh, Fish, PowerShell, and Elvish.
- **Documentation & Governance**:
  - Comprehensive CLI reference guide (`docs/cli-reference.md`).
  - Architecture and specification overview (`docs/architecture.md`).
  - Open source governance: `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`.

---

## [0.2.0] - 2026-10-12

### Added
- Interactive conversation picker using fuzzy terminal search.
- Compact export mode (`--compact`) filtering bloated system steps.
- Bundle inspection and metadata reporting via `agx info`.
- Direct conversation import into target Antigravity brain directories.

---

## [0.1.0] - 2026-10-10

### Added
- Initial proof-of-concept conversation archive tool.
- Basic `.agbundle` zip archive format with `manifest.json`.
