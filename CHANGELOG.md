# Changelog

All notable changes to the **Antigravity eXtension (`agx`)** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
