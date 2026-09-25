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
Inspect all Antigravity conversations recorded on your system:

```bash
# List recent conversations
agx list

# Filter conversations matching a workspace or project name
agx list --workspace "my-project" --limit 10
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
agx info my_session.agbundle
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

## Bundle File Structure (`.agbundle`)

An `.agbundle` is a Zstandard-compressed tar archive containing:

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
