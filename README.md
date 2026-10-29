# Xray Glass

رابط دسکتاپ لینوکس برای مدیریت Xray، sing-box، mihomo، Aether و MHR با Tauri و Rust. ساخت CI یک AppImage و بستهٔ deb تولید می‌کند و هسته‌های لازم را برای x64 و ARM64 بسته‌بندی می‌کند.

## اجرا و ساخت محلی

به Node.js، Rust و پیش‌نیازهای Tauri 2 (از جمله GTK/WebKitGTK 4.1) نیاز است. برای اجرای محلی، باینری‌های xray، sing-box، mihomo و aether را در `src-tauri/resources/` و پوشهٔ `mhr` را در `src-tauri/resources/mhr` قرار دهید، سپس اجرا کنید:

```sh
npm install
npx tauri dev
```

برای خروجی نهایی:

```sh
npx tauri build --bundles appimage,deb
```

خروجی‌ها در `src-tauri/target/release/bundle/` قرار می‌گیرند. workflow نسخه‌های سازگار هسته‌ها را برای x64 و ARM64 دریافت می‌کند.

## وضعیت پشتیبانی

پیوندهای VLESS، VMess، Trojan، Shadowsocks، SOCKS و HTTP به outboundهای Xray تبدیل می‌شوند، از جمله TLS/Reality و XTLS Vision. Hysteria2، TUIC و WireGuard را می‌توان از طریق پروفایل کامل sing-box JSON اجرا کرد. برای sing-box و mihomo، از Settings → Engine Profiles یک پروفایل بسازید و پیکربندی کامل بدهید؛ این هسته‌ها ورودی‌های محلی، DNS و routing را از همان فایل می‌سازند. مستندات رسمی: [sing-box outboundها](https://sing-box.sagernet.org/configuration/outbound/)، [TUIC](https://sing-box.sagernet.org/configuration/outbound/tuic/) و [mihomo routing](https://wiki.metacubex.one/en/config/rules/).

Proxy یک ورودی محلی SOCKS روی `127.0.0.1:10808` و HTTP روی `127.0.0.1:10809` اجرا می‌کند. TUN به مجوزهای شبکهٔ سیستم نیاز دارد؛ روی اکثر توزیع‌ها اجرای رابط به‌تنهایی مجوز root یا تنظیمات capabilities/systemd لازم را فراهم نمی‌کند. آزمون ping برای هر پروفایل یک نمونهٔ موقت Xray می‌سازد و زمان پاسخ HTTPS از مسیر SOCKS همان پروفایل را اندازه می‌گیرد؛ تست سادهٔ TCP فقط باز بودن پورت را نشان می‌دهد.

Aether یک هستهٔ جداست: پروفایل دستی `aether://` آن را روی `127.0.0.1:1819` اجرا می‌کند و Xray ورودی Proxy یا TUN را از SOCKS آن عبور می‌دهد. تنظیمات شامل MASQUE/WireGuard/GOOL/MIM، HTTP/3 و HTTP/2، scan، obfuscation، IP، DNS، SNI، ECH، fragmentation و exit country است. ثبت و نمایش identityها در Settings قرار دارد. Tor و Psiphon Shirokhorshid با حالت‌های chain/reverse/only، region و CDN override به گزینه‌های Aether متصل‌اند؛ Cloudflare Zero Trust هم team و روش‌های email/service token/JWT را می‌پذیرد. هویت‌ها و داده‌های Tor در پوشهٔ تنظیمات برنامه می‌مانند. مشخصات CLI در [راهنمای Aether](https://github.com/CluvexStudio/Aether/blob/main/Docs/DOCS.en.md) است.

MHR از کد MIT مرجع MehrON همراه برنامه بسته‌بندی شده است. یک پروفایل `mhr://` بسازید، Apps Script را منتشر کنید و Deployment ID و AUTH_KEY متناظر را در Settings ذخیره کنید. MHR سرویس محلی SOCKS می‌سازد و Xray ترافیک را از آن عبور می‌دهد؛ اعتبارها در فایل خصوصی تنظیمات برنامه ذخیره می‌شوند.

SNI Spoofing لینوکسی به‌صورت relay محلی پیاده‌سازی شده است. گزینهٔ آن را در Settings روشن کنید، Fake SNI را تعیین کنید و به یک پروفایل Xray با TLS عادی (VLESS/VMess/Trojan) وصل شوید. برنامه قبل از TLS واقعی یک ClientHello با SNI جعلی و sequence number خارج از پنجره تزریق می‌کند؛ جریان واقعی TLS بدون تغییر از relay عبور می‌کند. Connect IP/port اختیاری است و در حالت خالی از endpoint پروفایل استفاده می‌شود. این روش به `pkexec`, Python 3 و `CAP_NET_RAW` نیاز دارد و مجوز مدیریتی می‌گیرد. در این پیاده‌سازی IPv4 پشتیبانی می‌شود؛ Reality، Hysteria2/TUIC، Aether و کانفیگ‌های کامل sing-box/mihomo از SNI relay عبور نمی‌کنند. منطق تزریق از روش upstream MehrON (wrong sequence) پیروی می‌کند؛ backend ویندوزی WinDivert کپی نشده است.

تب Logs خروجی زندهٔ stdout و stderr هسته‌های Xray، sing-box، mihomo، Aether و MHR را با زمان، نام هسته و سطح پیام نشان می‌دهد. دکمهٔ SVG زنده، دنبال‌کردن خودکار انتهای لاگ را روشن/خاموش می‌کند؛ «پاک‌کردن» تاریخچهٔ همان نشست را پاک می‌کند. لاگ‌ها فعلاً بین اجرای برنامه‌ها ذخیره نمی‌شوند.



## بررسی PattN

فایل PattN ارسالی به‌عنوان مرجع بررسی شد. `SubscriptionHandler` لینک اصلی و لینک‌های اضافه را جداگانه می‌گیرد، Base64 را پیش از ترکیب decode می‌کند، و اگر دانلود پروکسی خالی/ناموفق باشد مسیر مستقیم را امتحان می‌کند؛ سپس فقط نتیجهٔ قابل‌پارْس را به‌عنوان پروفایل subscription ثبت می‌کند. این کلاینت در حالت `Auto` درخواست‌های مستقیم و SOCKS را همزمان می‌فرستد و اولین پاسخ دارای پروفایل معتبر را نگه می‌دارد؛ اگر پاسخ اول نامعتبر باشد، پاسخ مسیر دوم را هم بررسی می‌کند و فهرست قبلی را تا موفقیت کامل حفظ می‌کند. گزینه‌های دستی «با پروکسی» و «بدون پروکسی» هم برای منطبق‌شدن با منوی PattN باقی‌اند.

فرم subscription کلاینت اکنون User-Agent اختصاصی و چند URL اضافه (با خط جدید یا ویرگول جداشده) می‌پذیرد. هر URL جداگانه از مسیر Auto/Proxy/Direct دریافت می‌شود؛ URL ناموفق رد می‌شود و گروه فقط وقتی به‌روزرسانی می‌شود که دست‌کم یک پروفایل معتبر به دست آید. واردکردن آرایهٔ Shadowsocks در قالب SIP008 هم پشتیبانی می‌شود. این رفتار با `SubscriptionHandler` و `DownloadSubscriptionContent` در PattN هم‌راستا است.

مدل پروفایل PattN تفکیک endpoint، credentials، transport، security و protocol-specific extraها را نشان می‌دهد. این کلاینت همان فیلدهای قابل‌ویرایش، parser ورودی‌های لینک/JSON/Base64 و WireGuard، routeهای TUN، و اعتبارسنجی Xray را دارد. ورودی QR تصویر با BarcodeDetector در WebView فعال می‌شود، اگر runtime از آن پشتیبانی کند. Share LAN، SOCKS و HTTP را روی همهٔ رابط‌های شبکه باز می‌کند و احراز هویت اضافه نمی‌کند. پیاده‌سازی C# کپی نشده؛ پروتکل‌هایی که Xray پشتیبانی نمی‌کند در README بالا مشخص شده‌اند.

## مبنای فنی Xray

تنظیمات کلاینت Xray از بخش‌های `inbounds`، `outbounds`، `routing` و `dns` تشکیل می‌شود. هسته با `xray run -c config.json` اجرا می‌شود و `-test` برای اعتبارسنجی قبل از راه‌اندازی در دسترس است. مستندات رسمی: [Configuration](https://xtls.github.io/en/config/)، [Configure and Run](https://xtls.github.io/en/document/config.html)، [Command line](https://xtls.github.io/en/document/command)، [Transport Configuration](https://xtls.github.io/en/config/transport.html)، [TUN inbound](https://xtls.github.io/en/config/inbounds/tun.html) و [Routing](https://xtls.github.io/en/config/routing).
