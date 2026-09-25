---
name: agx
description: Antigravity Context & Brain eXchange tool. Backup, export, sanitize, inspect, or import Antigravity conversations, brains, and project artifacts. Activate this skill when the user asks to backup, export, share, or import an Antigravity chat session or project brain, or uses the /backup-session command.
---

# agx: Antigravity Context & Brain eXchange Skill

This skill allows the agent to backup, export, sanitize, inspect, and import Antigravity conversations and project brains so they can be preserved, archived, or shared with teammates across macOS, Linux, and Windows.

## Binary Location

The CLI tool `agx` is installed at:
- `agx` (in PATH or `~/.cargo/bin/agx`)
- Local build: `./target/release/agx` (in the `antigravity-backup` workspace)

If `agx` is not in the active PATH in a subshell, invoke it via:
```bash
~/.cargo/bin/agx <command>
```

---

## Workflows

### 1. Export Current Session (`/backup-session`)
When the user asks to backup or export the current session, or runs `/backup-session`:

1. Identify or auto-detect the current conversation for the active workspace:
   ```bash
   ~/.cargo/bin/agx export
   ```
   Or specify an output file:
   ```bash
   ~/.cargo/bin/agx export -o session_backup.agbundle
   ```

   **Compact Mode (`--compact` or `-c`)**:
   When the user wants a lightweight bundle to share quickly, use compact mode:
   ```bash
   ~/.cargo/bin/agx export --compact -o session_compact.agbundle
   ```
   *(Compact mode preserves implementation_plan.md, walkthrough.md, and transcript.jsonl, but drops heavy step logs, scratch files, and temporary media).*

   **Custom Ignore Rules (`.agignore`)**:
   `agx` automatically respects `.agignore` in the workspace or `~/.agignore` to exclude specific patterns (e.g. `*.mp4`, `.tempmediaStorage/`, `scratch/tmp_*`).

   **Interactive Picker (`-i`)**:
   Users can also pick any conversation interactively via fuzzy search:
   ```bash
   ~/.cargo/bin/agx export -i
   ```

2. If the user wants to export a specific conversation by ID:
   ```bash
   ~/.cargo/bin/agx export --id <CONVERSATION_ID> --compact -o <OUTPUT_NAME>.agbundle
   ```

3. Provide a clear summary to the user including:
   - Exported bundle file name and path
   - Number of brain artifacts packaged
   - Confirmation that secrets and tokens were sanitized
   - Instructions on how a colleague can import it on their machine

---

### 2. List Available Conversations
When the user asks to see previous sessions or find a session to export:

```bash
~/.cargo/bin/agx list
```

To filter by a project name or workspace path:
```bash
~/.cargo/bin/agx list --workspace "<keyword>" --limit 10
```

---

### 3. Inspect a Bundle (`info`)
When the user wants to know what is inside an `.agbundle` without importing it:

```bash
~/.cargo/bin/agx info <path-to-bundle.agbundle>
```

Report:
- Title and creation timestamp
- Source OS and author host
- Step count and brain artifacts count
- Sanitization status

---

### 4. Import a Bundle
When the user asks to import a colleague's session or restore a backup:

1. **Always preview first with `--dry-run`**:
   ```bash
   ~/.cargo/bin/agx import <path-to-bundle.agbundle> --dry-run
   ```

2. Confirm with the user and execute import, binding to the current or specified workspace:
   ```bash
   ~/.cargo/bin/agx import <path-to-bundle.agbundle> --target-workspace <workspace-path>
   ```

3. Inform the user that the session is now registered in Antigravity and visible in their conversation list.
