<div align="center">

<img src="assets/logo.png" alt="Antigravity eXtension (agx) - Formula 1 Style Logotype" width="360" />

# Antigravity eXtension (`agx`)

**Enterprise Context Exchange, Session Portability, and Security Engine for Google Antigravity**

[![Release](https://img.shields.io/badge/release-v1.0.0--rc.2-0052CC.svg?style=flat-square)](https://github.com/imsaba16/agx/releases)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-DEA584.svg?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache--2.0-28A745.svg?style=flat-square)](LICENSE)
[![Security Audit](https://img.shields.io/badge/security-audited-success.svg?style=flat-square)](SECURITY.md)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-555555.svg?style=flat-square)](https://github.com/imsaba16/agx)

<p align="center">
  <a href="#overview">Overview</a> &bull;
  <a href="#core-capabilities">Core Capabilities</a> &bull;
  <a href="#quickstart">Quickstart</a> &bull;
  <a href="#power-user-suite">Power-User Suite</a> &bull;
  <a href="#security-architecture">Security Architecture</a> &bull;
  <a href="#archive-specifications">Archive Formats</a> &bull;
  <a href="#frequently-asked-questions">FAQ</a> &bull;
  <a href="#documentation">Documentation</a>
</p>

</div>

---

## Overview

**Antigravity eXtension (`agx`)** is a high-performance command-line interface (CLI) and native agent skill built for **Google Antigravity**. It provides enterprise-grade session portability, context backup, cryptographic verification, and trajectory analysis across developer workstations and multi-agent environments.

By default, Google Antigravity stores conversation trajectories, planner thinking logs, tool execution records, and project brain artifacts locally inside `~/.gemini/antigravity`. When switching machines, collaborating with distributed teams, auditing agent execution steps, or reproducing bugs, local session context is trapped.

`agx` resolves this bottleneck by bundling single conversations into self-contained `.agbundle` packages, or entire collaborative environments into `.agproject` archives. Built in Rust with zero runtime dependencies, `agx` incorporates multi-threaded parallel compression, automatic secret redaction, path normalization, and in-place query capabilities without requiring archive decompression.

---

## Core Capabilities

- **Zero-Dependency Native Binary**: Built in Rust with Rayon-powered multi-threading and streaming I/O for instant packaging of multi-gigabyte project trajectories.
- **Cross-Platform Path Normalization**: Automatically converts absolute file URIs (`file:///Users/...`, `file:///home/...`, `file:///C:/...`) into portable `{{WORKSPACE_ROOT}}` tokens, enabling seamless transfers between macOS, Linux, and Windows.
- **Enterprise Security Hardening**: Proactive detection and masking of credentials (OpenAI, Gemini, Anthropic, AWS, GitHub tokens, SSH keys), Anti-Zip Slip directory protection, and decompression bomb mitigation.
- **Zero-Decompression Query Engine**: Search transcripts, grep thoughts, display syntax-highlighted files, or extract specific artifacts directly from archives without decompressing to disk.
- **Context & Trajectory Diffing**: Interactively compare step counts, token expenditures, artifact revisions, and implementation plans across sessions or against the live workspace.
- **Interactive Terminal Browser**: Full-screen split-pane TUI powered by `ratatui` with real-time Markdown preview and single-keystroke extraction.
- **Complete Workspace Synchronization**: Bundles multi-session histories alongside local customizations (`.agents/skills`, `.agent/rules`, and MCP server definitions).
- **Cryptographic Audit Pipeline**: Computes and validates SHA-256 integrity checksums and enforces canonical POSIX file permissions (`0o644`/`0o755`).

---

## Capability Comparison Matrix

| Feature | Standard Manual Copy | Generic Zip / Tar | Antigravity eXtension (`agx`) |
| :--- | :---: | :---: | :---: |
| Single Conversation Export | Partial | Manual | Automated via CLI & Skill |
| Cross-OS Path Remapping (`file:///`) | No | No | Yes (RFC Compliant Normalization) |
| Automated Secret & Token Masking | No | No | Yes (Regex Redaction Pipeline) |
| Zero-Decompression Search | No | No | Yes (`agx search`) |
| Session & Artifact Diffing | No | External tools | Built-in (`agx diff`) |
| Interactive TUI Session Explorer | No | No | Built-in (`agx explore`) |
| Skills, Rules & MCP Packaging | Manual | Manual | Unified (`agx project export`) |
| Protobuf Summaries Sync (`agyhub`) | No | No | Automatic Protobuf Wire Encoder |
| Automatic IDE & Daemon Refresh | No | No | Built-in (`agx restart` / `--restart`) |
| Cryptographic Integrity Verification | No | No | SHA-256 Hash Auditing (`agx verify`) |
| Anti-Zip Slip & Decompression Bomb Guard | No | Varies | Enforced by Default |

---

## Quickstart

### Installation

#### Homebrew (macOS & Linux)
```bash
brew install imsaba16/agx/agx
```

#### Scoop (Windows)
```powershell
scoop bucket add agx https://github.com/imsaba16/scoop-bucket
scoop install agx
```

#### Build and Install via Cargo
```bash
# Clone repository
git clone https://github.com/imsaba16/agx.git
cd agx

# Install the agx binary globally into ~/.cargo/bin
cargo install --path .
```

Verify your installation:
```bash
agx --version
# agx 1.0.0-rc.3
```

---

### Basic Usage Workflows

#### 1. Interactive Priority Dashboard
Launch the unified interactive menu designed with a prioritized, zero-clutter hierarchy:
```bash
agx
```
Presents 5 core actions without terminal overflow:
- **`Import Archive...`**: One-click restoration of `.agproject` or `.agbundle` archives.
- **`Export...`**: Consolidated menu to backup entire projects, active sessions, or browse past history.
- **`Interactive TUI Explorer`**: Full-screen split-pane session explorer with live markdown preview.
- **`Restart Antigravity`**: Gracefully refreshes the Antigravity language server daemon and loads all changes.
- **`More Tools & Utilities...`**: Secondary utilities (Diff, Search, Extract, Inspect, Verify, Sessions Table).

#### 2. Compact Session Export
Generate a lightweight archive excluding redundant planner steps and large binary payloads:
```bash
agx export <CONVERSATION_ID> --compact -o session_backup.agbundle
```

#### 3. Inspect Archive Contents
Read metadata, author system, step count, and artifact lists without extraction:
```bash
agx info session_backup.agbundle
```

#### 4. Import a Session on Another Workstation
Restore a session archive directly into your target Antigravity brain directory:
```bash
# Preview what will be imported without touching disk
agx import session_backup.agbundle --dry-run

# Import and bind to local workspace (safely forks ID by default)
agx import session_backup.agbundle --target-workspace /path/to/local/project

# Import and automatically restart Antigravity to reflect changes immediately
agx import session_backup.agbundle --target-workspace /path/to/local/project --restart
```

#### 5. Gracefully Restart Antigravity (`agx restart`)
Restart the Antigravity IDE and its background language server daemon to immediately reflect newly imported projects and conversations:
```bash
agx restart
# Or reopen a specific workspace:
agx restart --workspace /path/to/local/project
```

---

## Power-User Suite

`agx` provides command-line utilities designed for advanced debugging, auditing, and querying of agent trajectories.

### Zero-Decompression Search (`agx search`)
Query transcripts, model thoughts, tool executions, and artifacts directly inside archives without unpacking them to disk:
```bash
# Case-insensitive search with surrounding context lines
agx search project_archive.agproject "database migration" -i -C 2

# Regular expression pattern search across transcripts
agx search session.agbundle "error.*timeout" --regex

# Output machine-readable JSON for automated tooling
agx search session.agbundle "authentication" --json
```

### Visual Context Diffing (`agx diff`)
Compare two archives or evaluate an archive against your active workspace:
```bash
# Full side-by-side terminal diff of implementation plans
agx diff session_v1.agbundle session_v2.agbundle

# Summary statistics only (step count delta, file additions/deletions)
agx diff session_v1.agbundle session_v2.agbundle --stat
```

### Surgical File Extraction & Paging (`agx extract` & `agx cat`)
Isolate and extract individual artifacts without executing a full database restoration:
```bash
# Extract only the implementation plan artifact
agx extract session.agbundle artifacts/implementation_plan.md -o ./recovered/

# Stream transcript directly to stdout for shell pipeline processing
agx extract session.agbundle transcript.jsonl --stdout | jq '.[] | select(.type=="USER_INPUT")'

# View archived files with terminal syntax highlighting and line numbers
agx cat session.agbundle artifacts/walkthrough.md -n
```

### Interactive Terminal Explorer (`agx explore`)
Launch an interactive split-pane terminal dashboard to browse archive hierarchies:
```bash
agx explore project_archive.agproject
```
- Navigation: `Up` / `Down` / `j` / `k`
- Preview: Real-time Markdown rendering in adjacent pane
- Quick Extract: Press `e` to unpack selected item to working directory
- Exit: `q` or `Esc`

---

## Whole-Project Workspace Exchange (`agx project`)

Share an entire development environment in a single `.agproject` package, including all conversation trajectories, brain memory directories, and local customizations.

```bash
# Export the active workspace project
agx project export -o Workspace.agproject

# Inspect workspace package metadata and packaged customizations
agx project info Workspace.agproject

# Cryptographically audit archive integrity
agx verify Workspace.agproject

# Preview import with dry-run
agx project import Workspace.agproject --target-workspace ~/dev/my-project --dry-run

# Import into target directory and restart Antigravity immediately
agx project import Workspace.agproject --target-workspace ~/dev/my-project --restart
```

---

## Security Architecture

`agx` is engineered for production enterprise environments with strict security controls:

| Security Domain | Protective Mechanism | Enforcement Standard |
| :--- | :--- | :--- |
| **Path Traversal Protection** | Anti-Zip Slip Path Canonicalizer | Rejects relative directory escapes (`..`), null bytes, and malicious roots. |
| **Resource Exhaustion Defense** | Expansion Limit Thresholds | Blocks decompression bombs with a 5 GiB uncompressed ceiling and 100:1 ratio limit. |
| **Cryptographic Integrity** | SHA-256 Manifest Verification | Computes and validates cryptographic checksums for all archive contents via `agx verify`. |
| **Secret & Token Masking** | High-Entropy Regex Engine | Redacts API keys (OpenAI, Gemini, Anthropic), AWS credentials, GitHub PATs, and SSH/PEM private keys. |
| **Permission Sanitization** | Strict POSIX Bitmask | Strips setuid/setgid bits; standardizes files to `0o644` and directories to `0o755`. |

For security policies and vulnerability disclosure, see [SECURITY.md](SECURITY.md).

---

## Archive Specifications

### 1. Conversation Archive (`.agbundle`)
A ZIP container encapsulating a single conversation trajectory and its brain artifacts:
```
<session-id>.agbundle
├── manifest.json                  # Cryptographic manifest, metadata, hashes
├── transcript.jsonl               # Compact or full execution steps
├── transcript_full.jsonl          # Optional: Full uncompacted steps
└── artifacts/                     # Generated brain artifacts
    ├── architecture.png
    ├── implementation_plan.md
    └── walkthrough.md
```

### 2. Project Workspace Archive (`.agproject`)
A multi-session ZIP container packaging full project workspaces:
```
<project-name>.agproject
├── project_manifest.json          # Workspace metadata, hashes, settings
├── conversations/                 # Packaged conversation sessions
│   ├── <session-id-1>/
│   └── <session-id-2>/
└── customizations/                # Workspace customizations
    ├── skills/                    # .agents/skills/
    ├── rules/                     # .agent/rules/
    └── mcp/                       # MCP server configurations
```

Full architectural specifications are detailed in [docs/architecture.md](docs/architecture.md).

---

## Shell Autocompletion

Generate shell completions for your environment:

```bash
# Zsh
agx completion zsh > ~/.zsh/completion/_agx

# Bash
agx completion bash | sudo tee /etc/bash_completion.d/agx

# Fish
agx completion fish > ~/.config/fish/completions/agx.fish
```

---

## Native Antigravity Skill Integration

`agx` integrates directly into Google Antigravity as an agent skill. Trigger it naturally inside chat:
- `/backup-session`
- *"Export this conversation as a compact archive for my teammate"*
- *"Inspect and verify the bundle at ./backup.agbundle"*

Skill manifest: [`docs/SKILL.md`](docs/SKILL.md) (workspace: [`.agents/skills/agx/SKILL.md`](.agents/skills/agx/SKILL.md)).

---

## Frequently Asked Questions

### What is the difference between an `.agbundle` and an `.agproject` file?
An `.agbundle` encapsulates a single conversation session along with its specific brain artifacts. An `.agproject` file aggregates multiple conversations, project memory directories, and workspace-level customizations (skills, rules, and MCP definitions) into a unified archive for team onboarding and backup.

### How does `agx` handle absolute paths when importing across different operating systems?
During export, `agx` identifies machine-specific path prefixes (`file:///Users/...` on macOS, `file:///home/...` on Linux, `file:///C:/...` on Windows) and normalizes them into portable `{{WORKSPACE_ROOT}}` tokens. Upon import, `agx` dynamically binds these tokens to the recipient machine's local workspace directory.

### Will `agx export` leak API keys or sensitive environment variables?
No. By default, `agx` runs an automated secret-redaction scanner on all transcripts and text artifacts, replacing detected API keys, credentials, and tokens with safe placeholder tags (`[REDACTED_SECRET:<type>]`). Redaction can only be disabled by explicitly providing the `--no-sanitize` flag.

### Can I query or search archives without extracting them to disk?
Yes. The `agx search`, `agx cat`, and `agx extract` commands read directly from the archive's central directory and stream data in memory, allowing instant inspection and filtering without consuming filesystem storage.

---

## Documentation

- [CLI Reference Guide](docs/cli-reference.md): Detailed parameter, flag, and option breakdown for all subcommands.
- [Architecture & Format Specification](docs/architecture.md): Internal design, manifest schemas, and data structures.
- [Native Agent Skill Specification](docs/SKILL.md): Antigravity agent runbook, slash command triggers, and conversational workflows.
- [Packaging & Distribution Guide](docs/packaging-and-distribution.md): Setup instructions for Homebrew, Scoop, WinGet, and multi-platform binary releases.
- [Security Policy](SECURITY.md): Vulnerability reporting procedures and mitigation strategies.
- [Contributing Guide](CONTRIBUTING.md): Development environment setup, linting, and testing workflows.
- [Changelog](CHANGELOG.md): Release notes and version history.
- [Code of Conduct](CODE_OF_CONDUCT.md): Community guidelines and standards.

---

## License

Antigravity eXtension (`agx`) is open-source software licensed under the [Apache License 2.0](LICENSE).
