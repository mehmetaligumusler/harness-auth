<div align="center">

**English** | [Türkçe](README.tr.md)

# HarnessAuth

**Unified Windows desktop session manager for Claude Code, OpenCode, Antigravity CLI (`agy`), and ZCode/Z.AI.**

HarnessAuth stores all of your AI coding harness sessions, OAuth tokens, and API keys inside a single **Windows Credential Manager + AES-256-GCM** encrypted vault. Add unlimited accounts with your preferred providers, edit them, switch active methods in one click, or safely disable them without losing credentials — built with **Tauri v2 + Rust + React**.

</div>

---

## Features

### Claude Code (`claude`)
- **5-Method Active Switcher:**
  1. **Claude Subscription (OAuth):** Add multiple Anthropic Claude Pro/Max OAuth accounts via isolated `CLAUDE_CONFIG_DIR` (`claude auth login`) or import your live `~/.claude/.credentials.json` session.
  2. **ZCode Subscription (OAuth Proxy):** Pick any saved ZCode / BigModel OAuth account and model; the embedded background engine starts automatically and configures `~/.claude/settings.json`.
  3. **Z API (Direct API Key):** Connect directly to Z.AI (`https://api.z.ai/api/anthropic`) or BigModel without running a local proxy.
  4. **Provider API Key (`custom_api`):** Add, edit, and switch unlimited API accounts for Anthropic (official API), OpenRouter, DeepSeek, Z.AI, BigModel, or any custom provider ID + Base URL + model.
  5. **Disabled:** Safely clears active routing and credentials, auto-backing up any unsaved live session first.
- Preserves all unrelated user settings, hooks, permissions, and custom environment variables in `~/.claude/settings.json` and `~/.claude.json`.

### OpenCode (`opencode`)
- **4-Method Active Switcher:**
  1. **OpenCode Profile (OAuth / API):** Import live `%USERPROFILE%\.local\share\opencode\auth.json` + `opencode.json`, sign in via isolated `OPENCODE_AUTH_PATH` (`opencode auth login`), or add unlimited API profiles for Anthropic, OpenAI, Google, OpenRouter, Z.AI, BigModel, or any custom provider ID.
  2. **ZCode Subscription (OAuth Proxy):** Select from multiple ZCode OAuth accounts and a model to configure `provider.zcode` automatically.
  3. **Z API (Direct API Key):** Apply saved Z.AI / BigModel API keys directly to OpenCode without needing a local proxy.
  4. **Disabled:** Auto-backs up unsaved live credentials and cleans HarnessAuth-managed provider entries.
- Full create, edit, per-session enable/disable toggle, activate, and delete support.

### Antigravity CLI (`agy`)
- Manages Windows Credential Manager (`service: "gemini"`, `account: "antigravity"`) and the `%USERPROFILE%\.gemini\antigravity-cli\antigravity-oauth-token` mirror in the encrypted vault.
- Import your active `agy` session, add additional Google accounts without losing existing ones, inspect token expiry, switch accounts in one click, or disable.

### ZCode Accounts & Embedded Background Engine
- Manage multiple Z.AI / BigModel OAuth and API Key accounts (with optional custom Base URLs), per-account enable/disable toggles, and connection testing.
- Desktop-only architecture (no console/TUI or Docker); the ZCode OAuth bridge runs as a bundled headless sidecar (`harness-zcode-engine`).
- Coding Plan / Start Plan / Auto-Switch, live quotas, model testing, live logs, system tray, and Windows autostart.

---

## Installation

### End Users (Windows 10 / 11)
1. Download and run **`HarnessAuth_<version>_x64-setup.exe`** from [GitHub Releases](../../releases) (per-user install, no admin rights required).
2. No Bun, Node.js, or Docker installation is required — the background engine is bundled inside the installer.

### Data Layout & Security
- Application data is stored under `%APPDATA%\harness-auth` (`settings.json`, `accounts.json`, `sessions.json`, `proxy\`, `launches\`).
- All OAuth tokens and API keys are encrypted with **AES-256-GCM** using a master key stored in **Windows Credential Manager** (`HarnessAuth / master-key`). Raw secrets never cross the IPC boundary to the React UI.
- Existing accounts from legacy `%APPDATA%\zcode-manager` installations are migrated automatically on first launch.

---

## Development & Building the Installer

Prerequisites: **Bun 1.4+**, **Node.js 22+**, **pnpm 10+**, **Rust (stable, MSVC)**.

```powershell
# 1. Install dependencies
bun install
pnpm --dir desktop install

# 2. Build the embedded headless ZCode engine sidecar
bun run build:sidecar

# 3. Run the desktop app in development mode
bun run dev

# 4. Build the Windows NSIS installer (.exe)
bun run build:installer
```

Installer output: `desktop/src-tauri/target/release/bundle/nsis/HarnessAuth_1.0.0_x64-setup.exe`.

For desktop architecture details, see [desktop/README.md](desktop/README.md).

## Tests

```powershell
# Headless ZCode engine tests
bun test

# Desktop frontend typecheck + production build
pnpm --dir desktop build

# Rust backend unit tests (isolated temp dirs; never touches live user configs)
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib
```
