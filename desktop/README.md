# HarnessAuth — Masaüstü Uygulaması (`desktop/`)

**Claude Code**, **OpenCode**, **Antigravity CLI (`agy`)** ve **ZCode/Z.AI** için Windows 10/11 masaüstü oturum yöneticisi.

| Katman | Teknoloji |
|---|---|
| Arayüz | React 18 + TypeScript + Tailwind CSS + Zustand + Lucide |
| Uygulama çerçevesi | Tauri 2 (WebView2, NSIS installer) |
| Yerel backend | Rust (`harness-auth`: şifreli kasa, harness adaptörleri, keyring, süreç yönetimi) |
| Gömülü ZCode motoru | Headless `harness-zcode-engine.exe` (yan süreç / sidecar) |

---

## Mimari

- **Çoklu Harness Oturum Kasası (`src-tauri/src/harness/`):**
  - `claude_harness.rs`: `~/.claude/settings.json`, `~/.claude/.credentials.json` ve `~/.claude.json` için 5 yöntemli geçiş (`claude_oauth`, `zcode_oauth`, `z_api`, `custom_api`, `disabled`), izole `CLAUDE_CONFIG_DIR` ile yeni OAuth girişi ve otomatik yedekleme.
  - `opencode.rs`: `%USERPROFILE%\.local\share\opencode\auth.json` ve `%USERPROFILE%\.config\opencode\opencode.json` için 4 yöntemli geçiş (`profile`, `zcode_oauth`, `z_api`, `disabled`), sınırsız sağlayıcı API / ZCode köprüsü / OAuth oturumu ve düzenleme.
  - `antigravity.rs`: Windows Credential Manager (`gemini:antigravity`) ve `%USERPROFILE%\.gemini\antigravity-cli\antigravity-oauth-token` aynası üzerinden çoklu Google hesabı yönetimi.
- **ZCode Arka Plan Motoru (`src-tauri/src/proxy.rs`, `vault.rs`):**
  - `harness-zcode-engine.exe` yalnızca ZCode OAuth aboneliklerini yerel köprü üzerinden sunmak için gizli yan süreç olarak çalıştırılır; konsol/TUI açılmaz.
  - Uygulama kapanınca isteğe bağlı olarak arka planda çalışmaya devam eder ve yeniden açıldığında devralınır (adopt).

---

## Veri ve Güvenlik

- Tüm uygulama verisi `%APPDATA%\harness-auth` altında tutulur:
  - `settings.json`: arayüz ayarları ve aktif harness seçimleri
  - `accounts.json`: AES-256-GCM şifreli ZCode / Z.AI / BigModel hesap kasası
  - `sessions.json`: AES-256-GCM şifreli Claude Code / OpenCode / Antigravity oturum kasası
  - `proxy\config.yaml`, `proxy\store\credentials.json`, `proxy\errors.log`
  - `launches\`: terminal başlatma betikleri
  - `tmp\`: izole OAuth girişleri için geçici klasörler (işlem bitince silinir)
- Ana şifreleme anahtarı **Windows Credential Manager** (`HarnessAuth / master-key`) üzerinde saklanır ve frontend'e asla iletilmez.
- Tüm JSON dosya güncellemeleri geçici dosya + atomik `rename` ile yapılır; kullanıcının diğer ayarlarına dokunulmaz.

---

## Geliştirme ve Derleme

Önkoşullar: **Bun 1.4+**, **Node.js 22+**, **pnpm 10+**, **Rust (MSVC)**, WebView2.

```powershell
# Repo kökünden:
bun install
pnpm --dir desktop install

# Headless sidecar motorunu derle (src-tauri/binaries/ içine yazar)
bun run build:sidecar

# Geliştirme penceresini aç
bun run dev

# Windows NSIS kurulum dosyasını (.exe) derle
bun run build:installer
```

Üretilen çıktılar:
- **Installer:** `desktop/src-tauri/target/release/bundle/nsis/HarnessAuth_1.0.0_x64-setup.exe`
- **Portable:** `desktop/src-tauri/target/release/harness-auth.exe` (yanındaki `harness-zcode-engine.exe` ile)

## Testler

```powershell
# Frontend typecheck + build
pnpm --dir desktop build

# Rust backend birim testleri (tümü izole temp dizinlerinde çalışır)
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib

# Gerçek sidecar ile yaşam döngüsü E2E testi
cargo test --manifest-path desktop/src-tauri/Cargo.toml --test e2e
```
