# agx ⚡

**Antigravity Context & Brain eXchange**: A fast, lightweight, and cross-platform Rust CLI tool for exporting, sanitizing, and sharing Google Antigravity conversations, brains, and project artifacts across different computers and teammates.

---

## Why `agx`?

Google Antigravity keeps conversation transcripts, implementation plans, architectural decisions, and agent brainstorms on the local machine (`~/.gemini/antigravity`). When switching between laptops/desktops or collaborating with colleagues on the same project, that context is lost.

`agx` packages conversations and their brain artifacts into portable, self-contained `.agbundle` archives with:

* **Automatic Path Normalization**: Replaces machine-specific absolute file URIs (`file:///Users/...` or `file:///C:/...`) with `{{WORKSPACE_ROOT}}` placeholders so colleagues on other computers (Mac, Linux, Windows) can import sessions without path collisions.
* **Cross-OS Portability (Mac ↔ Linux ↔ Windows)**:
  - Formats internal TAR archives with universal POSIX forward slashes (`/`), preventing malformed extractions on Windows.
  - Generates valid RFC file URIs (`file:///C:/...` on Windows, `file:///home/...` on Linux, `file:///Users/...` on Mac).
  - Handles Windows verbatim UNC prefixes (`\\?\`), drive letters, and mixed slashes.
  - Automatically searches both `%USERPROFILE%\.gemini\antigravity` and `%APPDATA%\Gemini\antigravity` on Windows.
* **Zstandard (`zstd`) Compression**: 3–5x faster compression speeds and 20–30% smaller archives for JSON Lines and code diffs (with automatic backward-compatible Gzip fallback on import).
* **Parallel Processing with Rayon**: Multi-core file parsing, path remapping, and secret sanitization across all available CPU threads for instantaneous exports of large sessions.
* **Secret & Token Redaction**: Scans and masks OpenAI, Google, Anthropic, GitHub tokens, Bearer headers, and private key blocks before archiving.
* **Safe Forking**: Prevents overwriting local conversations when importing; safely generates a fresh collision-free UUID while retaining parent context and titles.
* **Interactive TUI Picker**: Fuzzy search through past conversations with arrow keys and live filtering.
* **Zero Dependencies**: Compiles to a single standalone binary.

---

## Installation

### From Source
```bash
# Clone and build
git clone <repo-url>
cd antigravity-backup

# Build optimized binary
cargo build --release

# Install globally to ~/.cargo/bin
cargo install --path .
```

---

## Usage

### 1. List Conversations
Inspect all Antigravity conversations recorded on your system in a clean, formatted Unicode table:

```bash
# List recent conversations in a formatted table
agx list

# Filter conversations matching a workspace or project name
agx list --workspace "my-project" --limit 10

# Output machine-readable JSON for scripting, CI/CD, or IDE plugins
agx list --json --limit 5
```

### 2. Export a Session & Brain to `.agbundle`
Export an active conversation along with its brain directory (`implementation_plan.md`, `walkthrough.md`, transcripts, scratch files):

```bash
# Auto-detect conversation for current directory
agx export

# Interactive TUI Picker (fuzzy search through past sessions with arrow keys)
agx export -i

# Compact Mode (ultra-lightweight, drops raw step logs & heavy media)
agx export --compact -o quick_share.agbundle

# Export a specific conversation by ID
agx export --id dd1457fe-2440-495a-a27f-d6f891bad9ff -o my_session.agbundle

# Export with custom workspace root for path remapping
agx export --workspace-root ~/dev/my-project -o feature_auth.agbundle
```

*By default, secrets and tokens are automatically sanitized (`--sanitize=true`).*

#### `.agignore` (Size & Privacy Filtering)
Create an `.agignore` file in your project root or `~/.agignore` to exclude specific files:

```gitignore
# Exclude video screen recordings
*.mp4
*.mov

# Exclude temporary media and scratch files
.tempmediaStorage/
scratch/tmp_*
```

### 3. Inspect a Bundle (`info`)
View metadata, author host, creation date, step count, and contents of any `.agbundle` without importing it:

```bash
# Human-readable summary
agx info my_session.agbundle

# Machine-readable JSON output
agx info my_session.agbundle --json
```

### 4. Import a Bundle on Another Machine
Import a teammate's `.agbundle` into your local Antigravity instance:

```bash
# Preview what will be imported without making changes
agx import my_session.agbundle --dry-run

# Import and bind to your local project folder
agx import my_session.agbundle --target-workspace ~/Documents/projects/my-repo

# Overwrite existing session instead of safe-forking
agx import my_session.agbundle --overwrite
```

---

### 5. Whole-Project Exchange (`agx project`)
Share the **entire** development context of a repository in one package: all conversations, all brain artifacts, custom skills (`.agents/skills`), custom rules (`.agents/rules`), and MCP server configs (`.agents/mcp_config.json`).

#### Export an Entire Project
```bash
# Export active workspace with all sessions, brains, skills, and MCP
agx project export

# Interactive session picker (select specific sessions to include)
agx project export -i

# Compact mode (strips bulky raw step logs across all sessions)
agx project export --compact -o my_full_project.agproject

# Skip workspace customizations if desired
agx project export --no-customizations
```

#### Inspect a Project Bundle
```bash
agx project info my_full_project.agproject
```
Displays:
* Project name, bundle version, creation timestamp, author host & OS
* Packaged conversations list (titles, step counts, last modified dates)
* Customizations included (skills count, rules, MCP servers)
* Total artifacts count and excluded count

#### Import an Entire Project
```bash
# Dry run preview
agx project import my_full_project.agproject --dry-run

# Import into current repository or a new target folder
agx project import my_full_project.agproject --target-workspace ~/dev/my-repo

# Overwrite existing sessions and customizations
agx project import my_full_project.agproject --overwrite
```

---

### 6. Shell Autocompletions (`completion`)
Generate native shell tab-completions for your shell:

```bash
# Zsh
agx completion zsh > ~/.zsh/completion/_agx

# Bash
agx completion bash > /etc/bash_completion.d/agx

# Fish
agx completion fish > ~/.config/fish/completions/agx.fish

# PowerShell
agx completion powershell >> $PROFILE
```

---

## Archive Formats

### Single Session (`.agbundle`)
```
feature_session.agbundle
├── manifest.json              # Source metadata, platform, author, original workspace paths
├── conversation.db            # SQLite conversation trajectory database
└── brain/                     # Project brain artifacts
    ├── implementation_plan.md # Architectural plan (paths remapped)
    ├── walkthrough.md         # Changes & test results
    ├── scratch/               # Helper & scratch scripts
    └── .system_generated/
        └── logs/
            └── transcript.jsonl # Complete chat history & tool execution logs
```

### Whole Project (`.agproject`)
```
my_repo.agproject
├── project_manifest.json      # Project manifest, sessions catalog, customizations summary
├── sessions/
│   ├── <uuid-1>/
│   │   ├── conversation.db    # SQLite trajectory for session 1
│   │   └── brain/             # Brain artifacts for session 1
│   └── <uuid-2>/
│       ├── conversation.db    # SQLite trajectory for session 2
│       └── brain/             # Brain artifacts for session 2
└── customizations/            # Project customizations
    ├── .agents/skills/        # Workspace skills & instructions
    ├── .agents/rules/         # Workspace rules
    └── .agents/mcp_config.json # Workspace MCP server configuration
```

---

## Native Antigravity Skill

`agx` is integrated directly as an Antigravity skill:
* **Workspace Skill**: `.agents/skills/agx/SKILL.md`
* **Global Skill**: `~/.gemini/antigravity/skills/agx/SKILL.md`

You can invoke it inside Antigravity by typing:
> **`/backup-session`** or *"Export this conversation as a compact bundle for Alice"*

---

## Development & Testing

```bash
# Run unit & integration tests
cargo test
```
