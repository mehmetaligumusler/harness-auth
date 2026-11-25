<div align="center">

**English** | [Türkçe](README.tr.md)

# HarnessAuth

**Centralized Windows desktop session & custom model manager for Claude Code, OpenCode, ZCode Desktop, and Antigravity CLI (`agy`).**

HarnessAuth stores all OAuth sessions, Z.AI/BigModel API keys, and `opencode.jsonc`-compatible custom providers inside a single **Windows Credential Manager + AES-256-GCM** encrypted vault (`vault.json`). Assign and run any compatible account or model in any harness with one click — shipped as a **single portable EXE** and **single NSIS installer** with the headless ZCode engine compressed and embedded directly inside `HarnessAuth.exe`.

</div>

---

## Highlights (v2 — Central Vault & Single EXE)

- **True Single-EXE Distribution:** The optional headless ZCode bridge engine is compiled, gzip-compressed, and embedded via `include_bytes!` inside `HarnessAuth.exe` (`engine.exe.gz`) — no secondary `.exe` beside the app.
- **3 Clean Tabs:**
  1. **Harnesses:** Instant **1-click account switching** directly inside each card (**Claude Code**, **OpenCode**, **ZCode Desktop**, **Antigravity CLI**), quick **`+ Add`** per harness, disable/launch buttons, and a live **`opencode.jsonc` Providers & Models** viewer/editor that preserves `skills` and `mcp`.
  2. **Accounts:** Unified vault for `claude_oauth`, `custom_provider` (`opencode.jsonc` multi-model schema with context/output limits), `antigravity_oauth`, `z_api`, and `zcode_oauth`. Supports one-click **Import from System** (`opencode.jsonc`, `~/.zcode/v2/`, `~/.claude`, `agy`), testing, editing, and enable/disable toggles.
  3. **Settings:** **General & System** (theme, language, default folder, tray/autostart, vault paths) plus an optional **ZCode Proxy & Quota** sub-tab (local bridge controls, live quota, and logs).

---

## Harness Compatibility Matrix

| Account Type | Claude Code | OpenCode (`opencode.jsonc`) | ZCode (`~/.zcode/v2/`) | Antigravity CLI (`agy`) |
|---|:---:|:---:|:---:|:---:|
| **ZCode OAuth** | ✅ (Embedded proxy) | ✅ (Embedded proxy) | ✅ (`oauth` mode) | — |
| **Z.AI / BigModel API Key** | ✅ (Direct Anthropic) | ✅ (`providers.harness-auth`) | ✅ (`standard-personal` rule) | — |
| **Claude OAuth** | ✅ (`~/.claude/.credentials.json`) | — | — | — |
| **Custom Provider (`opencode.jsonc`)** | ✅ (Anthropic & built-in OpenAI↔Anthropic Bridge) | ✅ (`providers.<slug>`) | — | — |
| **Antigravity OAuth** | — | — | — | ✅ (Credential Manager + mirror) |

---

## Development & Building

Prerequisites: **Bun 1.4+**, **Node.js 22+**, **pnpm 10+**, **Rust (stable, MSVC)**.

```powershell
# 1. Install dependencies
bun install
pnpm --dir desktop install

# 2. Compile & gzip-embed the headless ZCode engine
bun run build:sidecar

# 3. Run in development mode
bun run dev

# 4. Build single HarnessAuth.exe & NSIS installer
bun run build:installer
```

## Tests

```powershell
bun test
pnpm --dir desktop build
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib
```
