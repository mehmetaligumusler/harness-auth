# HarnessAuth — Masaüstü Uygulaması (`desktop/`)

**Claude Code**, **OpenCode**, **ZCode Desktop** ve **Antigravity CLI (`agy`)** için tek EXE Windows 10/11 merkezi oturum ve model yöneticisi.

| Katman | Teknoloji |
|---|---|
| Arayüz | React 18 + TypeScript + Tailwind CSS + Zustand + Lucide |
| Uygulama çerçevesi | Tauri 2 (WebView2, NSIS installer) |
| Yerel backend | Rust (`harness-auth`: `unified_vault`, `openai_bridge`, harness adaptörleri, keyring) |
| Gömülü ZCode motoru | `embedded/engine.exe.gz` (uygulama açılışında/gerektiğinde `%APPDATA%\harness-auth\runtime\` içine açılır) |

---

## Mimari (`src-tauri/src/`)

- **`unified_vault.rs`:** Tüm hesapları (`zcode_oauth`, `z_api`, `claude_oauth`, `antigravity_oauth`, `custom_provider`) `%APPDATA%\harness-auth\vault.json` dosyasında Windows Credential Manager + AES-256-GCM ile saklar. Eski `accounts.json` ve `sessions.json` dosyalarını ilk açılışta otomatik taşır.
- **`embedded_engine.rs`:** `desktop/scripts/prepare-sidecar.mjs` ile gzip-sıkıştırılan Bun ZCode motorunu `include_bytes!` üzerinden tek `HarnessAuth.exe` içine gömer.
- **Harness Adaptörleri (`src-tauri/src/harness/`):**
  - `claude_harness.rs`: `~/.claude/settings.json` ve `.credentials.json` yönetimi (OAuth, ZCode Proxy, Z.AI API, Custom Provider).
  - `openai_bridge.rs`: `@opencode/ai/providers/openai-compatible` özel modellerin Claude Code ile çalışabilmesi için yerleşik Anthropic `/v1/messages` ↔ OpenAI `/chat/completions` yerel köprüsü.
  - `opencode.rs`: JSONC parser ile `~/.config/opencode/opencode.jsonc` okuma/yazma (`providers`, `package`, `settings`, `models.<id>.limit`, `skills`/`mcp` koruma).
  - `zcode_harness.rs`: `~/.zcode/v2/` (`provider_config.json`, `setting.json`) içindeki `standard-personal` API kurallarını ve `oauth` modunu yönetir.
  - `antigravity.rs`: `gemini:antigravity` keyring + token mirror geçişi.

---

## Derleme ve Test

```powershell
# Gömülü motoru hazırla (desktop/src-tauri/embedded/engine.exe.gz)
bun run build:sidecar

# Frontend typecheck + build
pnpm --dir desktop build

# Rust backend birim testleri
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib

# Tek EXE & NSIS kurulum derlemesi
pnpm --dir desktop tauri build --bundles nsis
```
