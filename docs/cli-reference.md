# `agx` CLI Reference

The **Antigravity eXtension (`agx`)** command-line interface provides comprehensive tools to export, inspect, search, diff, verify, and import Antigravity conversations and complete project workspaces.

---

## Global Options & Environment Variables

These options are available on all `agx` invocations:

| Option | Flag | Description | Default |
| ------ | ---- | ----------- | ------- |
| `--app-dir <PATH>` | `-d` | Path to Antigravity application directory | Auto-detected: `$HOME/.gemini/antigravity` |
| `--non-interactive` | `-y` | Disable interactive prompts; fail if input required | `false` |
| `--verbose` | `-v` | Enable detailed logging output | `false` |
| `--help` | `-h` | Print help information for command or subcommand | - |
| `--version` | `-V` | Print version information (`agx 1.0.0-rc.2`) | - |

### Environment Variables

| Variable | Description |
| -------- | ----------- |
| `ANTIGRAVITY_DATA_DIR` | Overrides the default Antigravity app data path (equivalent to `--app-dir`). |
| `NO_COLOR` | When set (any non-empty value), disables ANSI color formatting. |
| `RUST_LOG` | Configures logging verbosity (e.g. `agx=debug`, `agx=trace`). |

---

## Subcommands Overview

| Command | Summary |
| ------- | ------- |
| [`agx list`](#agx-list) | List all available conversations in the Antigravity brain |
| [`agx export`](#agx-export) | Export a single conversation into an `.agbundle` archive |
| [`agx info`](#agx-info) | Inspect and display metadata from an `.agbundle` or `.agproject` file |
| [`agx import`](#agx-import) | Restore a conversation from an `.agbundle` into an active brain |
| [`agx restart`](#agx-restart) | Gracefully restart Antigravity and language server daemon |
| [`agx verify`](#agx-verify) | Run cryptographic SHA-256 integrity and security audits on archives |
| [`agx diff`](#agx-diff) | Compare two bundles or an archive against the current live workspace |
| [`agx search`](#agx-search) | Search archived transcripts, thoughts, and artifacts without extracting |
| [`agx extract`](#agx-extract) | Extract specific files or directories from an archive |
| [`agx cat`](#agx-cat) | Print an archived file directly to terminal with syntax highlighting |
| [`agx explore`](#agx-explore) | Interactive TUI browser to inspect and preview archive contents |
| [`agx project`](#agx-project) | Manage full-workspace `.agproject` bundles (multi-session + skills) |
| [`agx completion`](#agx-completion) | Generate shell autocompletion scripts for Bash, Zsh, Fish, etc. |

---

## Command Reference

### `agx list`

List all conversation sessions currently registered in the Antigravity application directory.

```bash
agx list [OPTIONS]
```

#### Options:
- `-l, --limit <LIMIT>`: Maximum number of conversations to display (default: `25`).
- `--json`: Output conversation list in structured JSON format for scripting.

#### Example:
```bash
# List the last 10 conversations
agx list --limit 10

# Output as JSON
agx list --json
```

---

### `agx export`

Export a specific conversation session and its associated brain artifacts into a portable `.agbundle` file.

```bash
agx export [OPTIONS] [CONVERSATION_ID]
```

#### Arguments:
- `[CONVERSATION_ID]`: Unique conversation UUID. If omitted in interactive mode, an interactive terminal fuzzy-picker is displayed.

#### Options:
- `-o, --output <PATH>`: Destination archive path. Defaults to `<conversation_id>.agbundle` or sanitized conversation title.
- `-c, --compact`: Compact export mode. Strips large base64 tool payloads and redundant planner steps, preserving only core prompts, responses, and artifacts.
- `--no-sanitize`: Disable automated secret masking. By default, API keys and credentials are automatically redacted.
- `--exclude-artifacts`: Exclude brain artifacts (images, generated code files, markdown logs) from the bundle, packing transcripts only.

#### Example:
```bash
# Export interactively
agx export

# Export a specific session with compact mode and custom output name
agx export dd1457fe-2440-495a-a27f-d6f891bad9ff -c -o ./backup-session.agbundle
```

---

### `agx info`

Inspect the contents, metadata, and manifest of an `.agbundle` or `.agproject` archive without extracting it to disk.

```bash
agx info [OPTIONS] <ARCHIVE_PATH>
```

#### Arguments:
- `<ARCHIVE_PATH>`: Path to the `.agbundle` or `.agproject` file.

#### Options:
- `--json`: Output bundle metadata in JSON format.
- `--list-files`: List every packaged file entry contained in the archive.

#### Example:
```bash
agx info ./backup-session.agbundle
agx info ./Project.agproject --json
```

---

### `agx import`

Restore a conversation from an `.agbundle` archive into the target Antigravity brain directory.

```bash
agx import [OPTIONS] <ARCHIVE_PATH>
```

#### Arguments:
- `<ARCHIVE_PATH>`: Path to the `.agbundle` file to import.

#### Options:
- `-t, --target-workspace <PATH>`: Target local workspace directory to rebind `{{WORKSPACE_ROOT}}` to (defaults to current working directory).
- `--overwrite`: Overwrite if conversation ID already exists locally (otherwise safely forks with a fresh UUID).
- `--dry-run`: Show what would be imported without modifying disk or database.
- `--restart` (alias: `--auto-restart`): Automatically restart Antigravity after import to reflect changes immediately.

#### Example:
```bash
# Preview import
agx import ./backup-session.agbundle --dry-run

# Import and bind to local workspace
agx import ./backup-session.agbundle --target-workspace /path/to/project

# Import and restart Antigravity automatically
agx import ./backup-session.agbundle --restart
```

---

### `agx restart`

Gracefully restart Antigravity and its background language server daemon to immediately reflect newly imported projects, sessions, or configuration changes.

```bash
agx restart [OPTIONS]
```

#### Options:
- `-w, --workspace <WORKSPACE>`: Optional workspace directory to reopen after restarting.

#### Platform Behavior:
- **macOS**: Gracefully quits Antigravity via AppleScript, waits for process termination, releases file locks, and relaunches via `open -a Antigravity` (restoring workspace tabs).
- **Linux**: Signals `SIGTERM` to the Antigravity process group, waits for socket release, and respawns the desktop process.
- **Windows**: Gracefully signals `Antigravity.exe` via `taskkill`, cleans lock handles, and respawns via `start Antigravity.exe`.

#### Example:
```bash
# Restart Antigravity
agx restart

# Restart and open specific workspace
agx restart --workspace /Users/dev/my-project
```

---

### `agx verify`

Performs an enterprise-grade cryptographic integrity and security audit on an `.agbundle` or `.agproject` file.

```bash
agx verify <ARCHIVE_PATH>
```

#### Audits Enforced:
1. **SHA-256 Cryptographic Checksums**: Confirms all extracted bytes match manifest hashes.
2. **Zip-Slip Prevention**: Verifies zero paths escape the archive destination boundary.
3. **Decompression Bomb Guard**: Validates uncompressed size ratios and total payload footprint.
4. **Permission Sanitization**: Verifies file and directory permissions match `0o644`/`0o755`.

#### Example:
```bash
agx verify ./PPE-React-Native.agproject
```

---

### `agx diff`

Compare two conversation archives, or compare an archive against another bundle to identify added/removed artifacts, modified steps, and content changes.

```bash
agx diff [OPTIONS] <ARCHIVE1> <ARCHIVE2>
```

#### Arguments:
- `<ARCHIVE1>`: First `.agbundle` file (base).
- `<ARCHIVE2>`: Second `.agbundle` file (target).

#### Options:
- `--stat`: Display summary statistics only (file counts, step differences) without full diffs.
- `--json`: Output diff report in JSON format.

#### Example:
```bash
agx diff ./session-v1.agbundle ./session-v2.agbundle
agx diff ./session-v1.agbundle ./session-v2.agbundle --stat
```

---

### `agx search`

Fast query engine that searches conversation transcripts, user inputs, assistant responses, model thoughts, and artifacts inside an archive without extraction.

```bash
agx search [OPTIONS] <ARCHIVE_PATH> <QUERY>
```

#### Arguments:
- `<ARCHIVE_PATH>`: Path to `.agbundle` or `.agproject`.
- `<QUERY>`: Search term or pattern.

#### Options:
- `-i, --ignore-case`: Perform case-insensitive search.
- `-r, --regex`: Treat `<QUERY>` as a regular expression.
- `-t, --transcript-only`: Search only conversation transcripts, skipping artifacts.
- `-a, --artifacts-only`: Search only artifacts, skipping conversation transcripts.
- `-C, --context <LINES>`: Number of context lines to display before and after each match (default: `2`).
- `--json`: Output search matches in JSON format.

#### Example:
```bash
agx search ./Project.agproject "authentication flow" -i -C 3
agx search ./session.agbundle "error.*timeout" --regex
```

---

### `agx extract`

Extract one or more specific files or subdirectories from an archive without unpacking the entire bundle.

```bash
agx extract [OPTIONS] <ARCHIVE_PATH> [TARGET_PATH]
```

#### Arguments:
- `<ARCHIVE_PATH>`: Path to `.agbundle` or `.agproject`.
- `[TARGET_PATH]`: Relative path of file or directory within the archive to extract (e.g. `transcript.jsonl` or `artifacts/`).

#### Options:
- `-o, --output-dir <DIR>`: Destination directory for extracted files (default: current directory).
- `--stdout`: Pipe the contents of a single file to `stdout` instead of saving to disk.

#### Example:
```bash
# Extract only the implementation plan artifact
agx extract ./session.agbundle artifacts/implementation_plan.md -o ./recovered/

# Stream transcript directly to jq
agx extract ./session.agbundle transcript.jsonl --stdout | jq '.[] | .type'
```

---

### `agx cat`

Direct terminal printing for any file contained in an archive, featuring automatic syntax highlighting and line numbers.

```bash
agx cat [OPTIONS] <ARCHIVE_PATH> <FILE_PATH>
```

#### Arguments:
- `<ARCHIVE_PATH>`: Path to `.agbundle` or `.agproject`.
- `<FILE_PATH>`: Relative path of the file inside the archive.

#### Options:
- `-n, --number`: Number all output lines.
- `--no-color`: Disable syntax highlighting and ANSI colors.

#### Example:
```bash
agx cat ./session.agbundle manifest.json
agx cat ./Project.agproject artifacts/walkthrough.md -n
```

---

### `agx explore`

Launches an interactive Terminal User Interface (TUI) to navigate the archive file tree, inspect manifests, preview transcripts and markdown artifacts live, and extract files with one keypress.

```bash
agx explore <ARCHIVE_PATH>
```

#### Controls:
- `↑ / ↓ / j / k`: Navigate file tree / scroll preview.
- `Enter`: Open file or toggle folder.
- `e`: Extract selected file to current directory.
- `q / Esc`: Exit explorer.

---

### `agx project`

Manage multi-conversation workspace bundles (`.agproject`), packaging complete agent brain memories, conversation histories, and project-specific skills (`.agents/skills`), rules (`.agent/rules`), and MCP configurations.

```bash
agx project <COMMAND>
```

#### Subcommands:

#### 1. `agx project export`
```bash
agx project export [OPTIONS]
```
- `-w, --workspace <PATH>`: Target workspace directory (default: current working directory).
- `-o, --output <PATH>`: Destination file (default: `<workspace_name>.agproject`).
- `-c, --compact`: Enable compact transcript compression.
- `--no-sanitize`: Disable secret masking.
- `--all-conversations`: Include all detected conversations without prompting.

#### 2. `agx project import`
```bash
agx project import [OPTIONS] <ARCHIVE_PATH>
```
- `-t, --target-workspace <PATH>`: Target local workspace directory to rebind the project and conversations to.
- `--overwrite`: Overwrite existing sessions and project customizations if present.
- `--no-customizations`: Skip restoring workspace customizations (`.agents/skills`, rules, MCP).
- `--dry-run`: Show what would be imported without making any changes to files or databases.
- `--restart` (alias: `--auto-restart`): Automatically restart Antigravity after import to reflect changes immediately.

#### Example:
```bash
# Preview project import
agx project import ./my-project.agproject --target-workspace ~/code/my-project --dry-run

# Import and restart Antigravity to reflect changes
agx project import ./my-project.agproject --target-workspace ~/code/my-project --restart
```

#### 3. `agx project info`
```bash
agx project info [OPTIONS] <ARCHIVE_PATH>
```
- `--json`: Output project bundle details as JSON.

---

### `agx completion`

Generate shell autocompletion script for your preferred shell.

```bash
agx completion <SHELL>
```

#### Arguments:
- `<SHELL>`: Target shell: `bash`, `zsh`, `fish`, `powershell`, or `elvish`.

#### Example Setup:
```bash
# Zsh
agx completion zsh > ~/.zsh/completion/_agx

# Bash
agx completion bash | sudo tee /etc/bash_completion.d/agx

# Fish
agx completion fish > ~/.config/fish/completions/agx.fish
```

### Interactive Dashboard (`agx`)

Running `agx` without subcommands launches an interactive terminal dashboard. The interface is organized in a priority-driven hierarchy:

```text
agx ─ Select an action:
❯ Import Archive...            Restore .agproject or .agbundle into Antigravity
  Export...                    Package current project or conversations
  Interactive TUI Explorer     Split-screen session navigator & live preview
  Restart Antigravity          Reload projects, conversations & language server
  More Tools & Utilities...    Diff, Search, Extract, Inspect & Verify
  Exit                         Close agx
```

- **`Import Archive...`**: Prompts for archive path and target workspace. Automatically detects if Antigravity is running and offers a graceful restart on completion.
- **`Export...`**: Opens a clean sub-choice:
  - *Export entire project (`.agproject`)*
  - *Export active conversation (`.agbundle`)*
  - *Browse & export past sessions*
- **`Interactive TUI Explorer`**: Full-screen split-pane navigation with real-time markdown rendering.
- **`Restart Antigravity`**: Shuts down and restarts Antigravity, reloading `language_server` and all stores.
- **`More Tools & Utilities...`**: Submenu housing `Diff`, `Search`, `Extract`, `Inspect`, `Verify`, and `List workspace sessions`.

---

## Exit Codes

| Code | Meaning |
| ---- | ------- |
| `0`  | Success |
| `1`  | General execution error / Command failure |
| `2`  | Invalid argument or CLI flag usage |
| `3`  | Security validation or cryptographic integrity failure |
| `4`  | Resource limit or decompression threshold exceeded |
