# ZCode Manager — Windows Masaüstü Uygulaması

ZCode Proxy'yi **tek bir uygulamadan** yöneten Windows 10/11 masaüstü arayüzü:
Z.AI OAuth hesapları, API Key bağlantıları, Coding/Start Plan, GLM modelleri,
proxy yaşam döngüsü ve Claude Code CLI entegrasyonu.

| Katman | Teknoloji |
|---|---|
| Arayüz | React 18 + TypeScript + Tailwind CSS + Lucide |
| Uygulama çerçevesi | Tauri 2 (WebView2) |
| Yerel backend | Rust (süreç yönetimi, kasa, şifreleme, panel istemcisi) |
| Proxy çekirdeği | Mevcut `zcode-proxy.exe` (yan süreç olarak, **değiştirilmedi**) |

Mimari prensip: **proxy çekirdeğine sıfır dokunuş.** Uygulama, proxy'yi kendi
kontrol ettiği ortam değişkenleriyle (`ZCODE_PROXY_CONFIG`,
`ZCODE_PROXY_STORE_DIR`, `ZCODE_PROXY_CREDENTIAL_SECRET`, `ZCODE_PANEL_*`)
izole bir yan süreç olarak çalıştırır ve mevcut panel kontrol API'sini
(`POST /api/control`) kullanır.

---

## Kurulum (son kullanıcı)

1. `ZCode Manager <sürüm> x64-setup.exe` dosyasını çalıştırın (yönetici
   hakkı gerekmez, kullanıcı başına kurulum). WebView2 runtime'ı yoksa
   kurulum sırasında indirilir.
2. **Bun veya Docker kurulu olmasına gerek yok** — proxy ikilisi
   (`zcode-proxy.exe`) uygulamanın yanında paketlenmiştir.

**Portable kullanım:** kurulum istemiyorsanız kurulumdan çıkan
`ZCode Manager.exe` + `zcode-proxy.exe` dosyalarını aynı klasöre koyup
doğrudan çalıştırın.

## İlk kullanım

1. Uygulamayı açın → **Hesaplar** sayfası.
2. İki bağlantı türünden birini ekleyin:
   - **OAuth ile giriş yap**: Tarayıcıda Z.AI/BigModel hesabınıza giriş
     yapın; oturum uygulama içinde otomatik tamamlanır. Kimlik bilgileri
     Windows Credential Manager tarafından korunan bir anahtarla
     şifrelenerek kasada saklanır.
   - **API Key ekle**: Z.AI veya BigModel API anahtarınızı girin.
     API Key bağlantıları Anthropic uyumlu (Coding Plan) endpoint'ini
     kullanır; standart anahtarlar **bakiyeden** ücretlendirilir ve Coding
     Plan kotanızı kullanmaz.
3. İlk hesap otomatik etkinleşir. **Panel** sayfasından
   **Proxy'yi Başlat**'ın.

## Günlük kullanım

- **Panel**: proxy durumu, aktif hesap/plan/model, gerçek kota kartları,
  son istekler ve hatalar. "Claude Code'u Başlat" tek tıkla çalışır.
- **Hesaplar**: hesap ekle/sil/yeniden adlandır, bağlantı testi (0 maliyetli
  `/api/paas/v4/models` sorgusu), tek tıkla hesap değiştirme (proxy seçili
  hesapla yeniden başlar).
- **Planlar ve Kotalar**: Coding Plan / Start Plan / Otomatik Geçiş
  (`planAutoSwitch` + `planPriority`). API Key hesaplarında Start Plan
  kullanılamaz (JWT gerektirir) — arayüz bunu açıkça belirtir.
- **Modeller**: katalog + proxy'nin listelediği modeller + özel model adları;
  varsayılan model seçimi, gerçek 1-token'lık erişim testi.
- **Claude Code**: CLI algılama (kurulu değilse yönergeler), Proxy/Normal
  mod seçimi, model + proje klasörü ile yeni terminal penceresinde başlatma.
  Yalnızca açılan pencerenin ortam değişkenleri ayarlanır
  (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_MODEL`, …);
  genel Claude ayarlarınız değişmez. Normal mod tamamen temiz bir ortamla
  açılır.
- **Proxy Yönetimi**: sunucu başlat/durdur (süreç ayakta kalır; kota ve
  loglar erişilebilir olur), süreç yeniden başlatma, port/host/anahtar
  değişimi (otomatik yeniden başlatma), bağlantı testi.
- **Loglar**: canlı panel logları (2 sn'de bir) + `errors.log` hata
  kayıtları, seviye filtreleriyle.
- **Ayarlar**: tema (koyu/açık), dil (TR/EN), varsayılanlar, Windows
  açılışında başlatma, tepsiye küçültme, uygulama kapalıyken proxy'yi
  çalışır tutma, proxy otomatik başlatma.

## Sistem tepsisi

- Pencere kapatma düğmesi uygulamayı tepsiye küçültür (Ayarlar'dan
  değiştirilebilir).
- Tepsi sol tık: pencereyi göster. Menü: göster / proxy aç-kapat / çıkış.
- Uygulama kapansa da proxy varsayılan olarak çalışmaya devam eder; uygulama
  yeniden açıldığında süreç otomatik devralınır (adopt).

## Veri ve güvenlik

- Tüm veri `%APPDATA%\zcode-manager` altında:
  `settings.json`, `accounts.json` (şifreli hesap kasası),
  `proxy\config.yaml`, `proxy\store\credentials.json` (proxy'nin AES-256-GCM
  deposu), `proxy\errors.log`.
- Kasa anahtarı ve proxy credential secret'i **Windows Credential
  Manager**'da saklanır; frontend'e asla gönderilmez, loglarda gösterilmez.
- Panel yalnızca `127.0.0.1`'e bağlanır ve rastgele üretilmiş token ile
  korunur. Proxy varsayılan olarak `127.0.0.1` + rastgele istemci anahtarıyla
  başlar (Ayarlar → Proxy Yönetimi'nden değiştirilebilir).
- Claude Code başlatma betikleri katı doğrulamadan geçer; model adları
  `[A-Za-z0-9._\[\]-]` dışında karakter içeremez (komut enjeksiyonu yok).

## Geliştirme

Önkoşullar: Node 18+, pnpm, Rust (MSVC), WebView2 SDK'ları (VS 2022 C++).

```bash
# Proxy ikilisi (repo kökünden)
bun install
bun run build            # -> zcode-proxy.exe

# Masaüstü uygulaması
cd desktop
pnpm install
pnpm prepare:sidecar     # exe'yi src-tauri/binaries'e kopyalar
pnpm tauri dev           # geliştirme penceresi
pnpm tauri build         # NSIS installer + portable exe
```

Çıktılar:

- Installer: `src-tauri/target/release/bundle/nsis/ZCode Manager_1.0.0_x64-setup.exe`
- Portable: `src-tauri/target/release/zcode-manager.exe`
  (+ yanındaki `zcode-proxy.exe`)

### Testler

```bash
cd desktop/src-tauri
cargo test               # birim testler (kripto, yaml yama, kasa, betik üretimi…)
cargo test --test e2e    # GERÇEK proxy ikilisiyle yaşam döngüsü testi
# Gerçek Z.AI hesabıyla (host ZCode kurulumu gerekir):
ZCM_E2E_REAL=1 cargo test --test e2e real_account_e2e -- --ignored
```

`real_account_e2e` gerçek kota snapshot'ı çeker ve 1 token'lık gerçek GLM
üretimi yapar.
