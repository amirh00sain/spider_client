# Xray Glass

رابط دسکتاپ لینوکس برای مدیریت پیوندهای Xray با Tauri و Rust. ساخت CI یک AppImage و بستهٔ deb تولید می‌کند و Xray را هنگام build از release رسمی XTLS دریافت می‌کند.

## اجرا و ساخت محلی

به Node.js، Rust و پیش‌نیازهای Tauri 2 روی لینوکس نیاز است. فایل اجرایی Xray را در `src-tauri/resources/xray` قرار دهید، سپس اجرا کنید:

```sh
npm install
npx tauri dev
```

برای خروجی نهایی:

```sh
npx tauri build --bundles appimage,deb
```

خروجی‌ها در `src-tauri/target/release/bundle/` قرار می‌گیرند. workflow در `.github/workflows/build.yml` باینری هسته را از آخرین release رسمی دریافت می‌کند.

## وضعیت پشتیبانی

پیوندهای VLESS، VMess، Trojan، Shadowsocks، SOCKS، HTTP و Hysteria2 به outbound هسته تبدیل می‌شوند. برای WireGuard می‌توان outbound کامل JSON را وارد و ویرایش کرد. SSH، chain، dnstt و masterdns در Xray outbound بومی ندارند؛ این موارد به‌صورت URI/JSON نگهداری می‌شوند ولی با هستهٔ Xray این برنامه وصل نمی‌شوند. فهرست رسمی outboundها را در [مستندات Xray](https://xtls.github.io/en/config/outbounds/) ببینید.

Proxy یک ورودی محلی SOCKS روی `127.0.0.1:10808` و HTTP روی `127.0.0.1:10809` اجرا می‌کند. TUN به مجوزهای شبکهٔ سیستم نیاز دارد؛ روی اکثر توزیع‌ها اجرای رابط به‌تنهایی مجوز root یا تنظیمات capabilities/systemd لازم را فراهم نمی‌کند.

## مبنای فنی Xray

تنظیمات کلاینت Xray از بخش‌های `inbounds`، `outbounds`، `routing` و `dns` تشکیل می‌شود. هسته با `xray run -c config.json` اجرا می‌شود و `-test` برای اعتبارسنجی قبل از راه‌اندازی در دسترس است. مستندات رسمی: [Configuration](https://xtls.github.io/en/config/)، [Configure and Run](https://xtls.github.io/en/document/config.html)، [Command line](https://xtls.github.io/en/document/command)، [Transport Configuration](https://xtls.github.io/en/config/transport.html)، [TUN inbound](https://xtls.github.io/en/config/inbounds/tun.html) و [Routing](https://xtls.github.io/en/config/routing).
