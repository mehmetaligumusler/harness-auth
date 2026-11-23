<div align="center">

[English](README.md) | **Türkçe**

# HarnessAuth

**Claude Code, OpenCode, Antigravity CLI (`agy`) ve ZCode/Z.AI oturumlarını tek bir masaüstü uygulamasından yönetin.**

HarnessAuth, yapay zeka kodlama araçlarınızın (AI harnesses) tüm oturumlarını, OAuth token'larını ve API anahtarlarını **Windows Credential Manager + AES-256-GCM** ile şifrelenmiş tek bir kasada saklayan; istediğiniz sağlayıcıyla sınırsız hesap eklemenizi, düzenlemenizi, tek tıkla geçiş yapmanızı ve devre dışı bırakmanızı (disable) sağlayan **Windows masaüstü uygulamasıdır** (Tauri v2 + Rust + React).

</div>

---

## Öne Çıkan Özellikler

### Claude Code (`claude`)
- **5 Aktif Yöntem Arasında Tek Tıkla Geçiş:**
  1. **Claude Aboneliği (OAuth):** Çoklu Anthropic Claude Pro/Max OAuth oturumlarını izole `CLAUDE_CONFIG_DIR` ile uygulama içinden ekleyin (`claude auth login`) veya mevcut `~/.claude/.credentials.json` oturumunuzu içe aktarın.
  2. **ZCode Aboneliği (OAuth Proxy):** Birden fazla ZCode / BigModel OAuth hesabından istediğinizi ve modelini seçin; gömülü arka plan motoru otomatik başlar ve `~/.claude/settings.json` yönlendirilir.
  3. **Z API (Doğrudan API Key):** Arka plan proxy'si çalıştırmadan doğrudan Z.AI (`https://api.z.ai/api/anthropic`) veya BigModel endpoint'ine bağlanın.
  4. **Sağlayıcı API Key (`custom_api`):** Anthropic (Resmi API), OpenRouter, DeepSeek, Z.AI, BigModel veya tamamen özel bir sağlayıcı kimliği + Base URL + model ile sınırsız API hesabı ekleyin, düzenleyin ve aktif edin.
  5. **Devre Dışı (Disable):** Aktif yönlendirmeyi ve kimlik dosyasını temizler; kasada kaydı olmayan canlı bir oturum varsa silmeden önce otomatik yedekler.
- Global `~/.claude/settings.json` ve `~/.claude.json` dosyalarındaki mevcut hook, izin, tema ve özel env değişkenlerinize dokunmaz.

### OpenCode (`opencode`)
- **4 Aktif Yöntem Arasında Tek Tıkla Geçiş:**
  1. **OpenCode Profili (OAuth / API):** Mevcut `%USERPROFILE%\.local\share\opencode\auth.json` + `opencode.json` oturumunu içe aktarın, izole `OPENCODE_AUTH_PATH` ile `opencode auth login` yapın veya Anthropic, OpenAI, Google, OpenRouter, Z.AI, BigModel ya da özel sağlayıcı kimliğiyle sınırsız API profili ekleyin.
  2. **ZCode Aboneliği (OAuth Proxy):** Kayıtlı ZCode OAuth hesaplarınızdan istediğinizi + modeli seçip doğrudan OpenCode `provider.zcode` olarak aktif edin.
  3. **Z API (Doğrudan API Key):** Kayıtlı Z.AI / BigModel API hesaplarını proxy gerekmeden doğrudan OpenCode içine yazın.
  4. **Devre Dışı (Disable):** Canlı `auth.json` yedeğini koruyarak aktif oturumu ve HarnessAuth tarafından yönetilen provider girdilerini temizler.
- Tüm kayıtlı API ve ZCode köprü profilleri düzenlenebilir, tek tek etkinleştirilip devre dışı bırakılabilir (enable/disable) ve silinebilir.

### Antigravity CLI (`agy`)
- Windows Credential Manager (`service: "gemini"`, `account: "antigravity"`) ve `%USERPROFILE%\.gemini\antigravity-cli\antigravity-oauth-token` dosya aynasını şifreli kasada yönetir.
- Mevcut `agy` oturumunu içe aktarabilir, mevcut oturumu kaybetmeden yeni Google hesabı ekleyebilir, token geçerlilik süresini görebilir, hesaplar arası tek tıkla geçiş yapabilir veya oturumu devre dışı bırakabilirsiniz.

### ZCode Hesapları & Gömülü Arka Plan Servisi
- Z.AI / BigModel OAuth ve API Key hesapları (opsiyonel özel Base URL desteğiyle), hesap bazlı **Etkin / Devre Dışı** anahtarı ve bağlantı testi.
- Konsol/TUI arayüzü ve Docker kaldırılmıştır; ZCode OAuth köprüsü yalnızca uygulamanın içine gömülü gizli arka plan motoru (`harness-zcode-engine`) olarak çalışır.
- Coding Plan / Start Plan / Otomatik Geçiş, canlı kota takibi, model testi, canlı loglar, sistem tepsisi ve Windows açılışında otomatik başlatma.

---

## Kurulum

### Son Kullanıcı (Windows 10 / 11)
1. [GitHub Releases](../../releases) üzerinden **`HarnessAuth_<sürüm>_x64-setup.exe`** kurulum dosyasını indirin ve çalıştırın (yönetici izni gerektirmez).
2. Ek olarak Bun, Node.js veya Docker kurmanıza gerek yoktur; arka plan motoru kurulum paketinin içinde gelir.

### Veri Konumu ve Güvenlik
- Uygulama verileri `%APPDATA%\harness-auth` klasöründe tutulur (`settings.json`, `accounts.json`, `sessions.json`, `proxy\`, `launches\`).
- Tüm OAuth token'ları ve API anahtarları **Windows Credential Manager** (`HarnessAuth / master-key`) üzerinde saklanan ana anahtarla **AES-256-GCM** şifrelenir; arayüz (frontend) katmanına asla ham sır gönderilmez.
- Önceki sürümden (`%APPDATA%\zcode-manager`) yükselten kullanıcıların ZCode hesapları ilk açılışta otomatik olarak yeni kasaya taşınır.

---

## Geliştirme ve Paketleme (Build)

Gereksinimler: **Bun 1.4+**, **Node.js 22+**, **pnpm 10+**, **Rust (stable, MSVC)**.

```powershell
# 1. Bağımlılıkları yükle
bun install
pnpm --dir desktop install

# 2. Gömülü headless ZCode motorunu derle
bun run build:sidecar

# 3. Masaüstü uygulamasını geliştirme modunda başlat
bun run dev

# 4. Windows NSIS kurulum dosyasını (.exe) derle
bun run build:installer
```

Üretilen kurulum paketi: `desktop/src-tauri/target/release/bundle/nsis/HarnessAuth_1.0.0_x64-setup.exe`.

Ayrıntılı masaüstü mimarisi için [desktop/README.md](desktop/README.md) dosyasına bakabilirsiniz.

## Testler

```powershell
# Headless ZCode motor testleri
bun test

# Masaüstü frontend typecheck + production build
pnpm --dir desktop build

# Rust backend birim testleri (gerçek kullanıcı dosyalarına dokunmaz)
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib
```
