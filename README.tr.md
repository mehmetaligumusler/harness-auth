<div align="center">

[English](README.md) | **Türkçe**

# HarnessAuth

**Claude Code, OpenCode, ZCode Desktop ve Antigravity CLI (`agy`) hesap ve özel modellerini tek bir merkezi kasadan yönetin.**

HarnessAuth, tüm AI kodlama araçlarınızın OAuth oturumlarını, Z.AI/BigModel API anahtarlarını ve `opencode.jsonc` uyumlu özel modellerini **Windows Credential Manager + AES-256-GCM** ile şifrelenmiş tek bir kasada (`vault.json`) saklar; istediğiniz hesap veya modeli istediğiniz araçta tek tıkla çalıştırmanızı sağlar. ZCode OAuth köprüsü ayrı bir yan EXE olmadan doğrudan uygulamanın içine gömülü **tek EXE** ve **tek kurulum dosyası** olarak dağıtılır.

</div>

---

## Yenilikler (v2 — Merkezi Kasa & Tek EXE)

- **Tek Dosya Mimari:** Gömülü ZCode köprü motoru `HarnessAuth.exe` içine sıkıştırılarak paketlenir; kurulum veya taşınabilir kopyada yanında ikinci bir `.exe` dosyası bulunmaz.
- **3 Sadelik Odaklı Sekme:**
  1. **Araçlar (`Harnesses`):** Claude Code, OpenCode, ZCode Desktop ve Antigravity CLI (`agy`) kartları içinde her aracın uyumlu hesapları listelenir; istediğiniz hesaba tek tıkla anında geçebilir, `+ Ekle` ile doğrudan hesap/model ekleyebilir, devre dışı bırakabilir veya projede başlatabilirsiniz. Canlı **`opencode.jsonc` Modelleri** paneliyle OpenCode sağlayıcılarını (`providers`, `package`, `settings.baseURL/apiKey`, `models.<id>.limit`) uygulama içinden yönetebilirsiniz (`skills` ve `mcp` ayarları korunur).
  2. **Hesaplar (`Accounts`):** `claude_oauth`, `custom_provider` (`opencode.jsonc` uyumlu çoklu model), `antigravity_oauth`, `z_api` ve `zcode_oauth` kayıtlarını tek ekranda ekleyip düzenleyebilir, test edebilir, etkin/devre dışı yapabilir veya **Sistemden İçe Aktar** ile mevcut oturumları otomatik içeri alabilirsiniz.
  3. **Ayarlar (`Settings`):** **Genel & Sistem** (tema, dil, varsayılan klasör, tepsiye küçültme, Windows başlangıcı, kasa yolları) ve isteğe bağlı **ZCode Proxy & Kota** (gömülü proxy köprüsü kontrolü, kotalar ve loglar).

---

## Araç Desteği Matrisi

| Hesap / Yöntem | Claude Code | OpenCode (`opencode.jsonc`) | ZCode (`~/.zcode/v2/`) | Antigravity CLI (`agy`) |
|---|:---:|:---:|:---:|:---:|
| **ZCode OAuth** | ✅ (Gömülü proxy) | ✅ (Gömülü proxy) | ✅ (`oauth` modu) | — |
| **Z.AI / BigModel API Key** | ✅ (Doğrudan Anthropic) | ✅ (`providers.harness-auth`) | ✅ (`standard-personal` kuralı) | — |
| **Claude OAuth** | ✅ (`~/.claude/.credentials.json`) | — | — | — |
| **Özel Model (`opencode.jsonc`)** | ✅ (Anthropic & yerleşik OpenAI↔Anthropic Bridge) | ✅ (`providers.<slug>`) | — | — |
| **Antigravity OAuth** | — | — | — | ✅ (Credential Manager + mirror) |

---

## Kurulum & Taşınabilir Kullanım (Windows 10 / 11)

- **Kurulum Dosyası:** `HarnessAuth_1.0.0_x64-setup.exe` (kullanıcı başına kurulum, yönetici izni gerektirmez).
- **Tek Taşınabilir EXE:** `HarnessAuth.exe` — kurulum gerektirmez, yanında ek `.exe` dosyası taşımaz.
- **Güvenlik:** Tüm token ve API anahtarları `%APPDATA%\harness-auth\vault.json` içinde Windows Credential Manager anahtarıyla (`HarnessAuth / master-key`) AES-256-GCM şifrelenir. Eski `accounts.json` ve `sessions.json` kayıtları ilk açılışta otomatik olarak merkezi kasaya taşınır.

---

## Geliştirme ve Derleme

Gereksinimler: **Bun 1.4+**, **Node.js 22+**, **pnpm 10+**, **Rust (stable, MSVC)**.

```powershell
# 1. Bağımlılıkları yükle
bun install
pnpm --dir desktop install

# 2. Gömülü ZCode motorunu derle ve sıkıştır (desktop/src-tauri/embedded/engine.exe.gz)
bun run build:sidecar

# 3. Geliştirme modunda başlat
bun run dev

# 4. Tek EXE + NSIS kurulum dosyasını derle
bun run build:installer
```

## Testler

```powershell
# Headless ZCode motor testleri
bun test

# Frontend typecheck + Vite build
pnpm --dir desktop build

# Rust birim testleri (tümü izole temp dizinlerinde çalışır)
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib
```
