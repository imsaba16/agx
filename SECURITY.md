# Security Policy

The **Antigravity eXtension (`agx`)** project takes the security of agentic environments, conversation archives, and project contexts seriously.

---

## Supported Versions

Only the latest release candidate and stable major releases receive active security patches.

| Version      | Supported          |
| ------------ | ------------------ |
| `1.0.0-rc.x` | :white_check_mark: |
| `< 1.0.0`    | :x:                |

---

## Reporting a Vulnerability

If you discover a security vulnerability in `agx`, **please do not open a public issue.** Public disclosure puts users and developer environments at risk.

Instead, report security vulnerabilities through:
- **GitHub Security Advisory**: [Create a private advisory](https://github.com/imsaba16/agx/security/advisories/new)
- **Security Email**: Contact the security maintainers at `security@antigravity.dev` or `imsaba16@gmail.com`.

### What to Include in Your Report

To help us triage and resolve the issue quickly, please include:
1. **Description**: Clear description of the vulnerability and attack vector.
2. **Steps to Reproduce**: Minimal reproduction script, proof of concept archive (`.agbundle` or `.agproject`), or command sequence.
3. **Impact**: Potential consequences (e.g., arbitrary file overwrite, credential leak, zip bomb denial of service).
4. **Environment**: Operating system, architecture, and `agx --version`.

### Our Security Response Process

- **Acknowledgment**: Within 48 hours of receipt.
- **Assessment**: Severity and vulnerability scope evaluation within 5 business days.
- **Fix & Disclosure**: A security patch will be prepared and tested. We coordinate responsible disclosure alongside CVE publication if applicable.

---

## Built-In Security Architecture

`agx` is engineered with multi-layered defense-in-depth:

### 1. Zero Path Traversal (Anti-Zip Slip)
`agx` rejects any path within an archive that attempts relative directory escape (`..`), absolute roots (`/` or `C:\`), or malicious Windows drive prefixes. Extraction paths are strictly anchored to the designated destination root.

### 2. Decompression Bomb & Resource Exhaustion Protection
Before and during unpacking, `agx` enforces:
- Max uncompressed archive expansion limits (default: 5 GB).
- Compression ratio safeguards to prevent recursive memory amplification attacks.
- Individual file size maximum checks.

### 3. Automatic Secret Masking & Redaction
Conversation transcripts, tool outputs, and environment variables often contain secrets. During export, `agx` automatically inspects content against regex patterns for:
- API Keys: Google Cloud / Gemini (`AIza[0-9A-Za-z\\-_]{35}`), OpenAI (`sk-[a-zA-Z0-9]{48}`), Anthropic, GitHub tokens (`gh[pous]_[A-Za-z0-9_]{36,255}`).
- Cloud Credentials: AWS Access Keys (`AKIA[0-9A-Z]{16}`), AWS Secret Keys.
- Private Keys: RSA, DSA, EC, OpenSSH PEM blocks.
- Generic bearer tokens, high-entropy secrets, and JSON web tokens (JWT).

Secrets are securely redacted to `[REDACTED_SECRET:<type>]` unless `--no-sanitize` is explicitly provided.

### 4. Cryptographic Integrity Auditing (`agx verify`)
Every archive packages a cryptographic manifest with individual file SHA-256 hashes. Running `agx verify <archive>` validates:
- Hash consistency against archive contents.
- Strict canonical permission bits (`0o644` for files, `0o755` for directories).
- Structural integrity of the manifest JSON.
