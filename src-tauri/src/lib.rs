use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::Stdio};
use tauri::{Manager, State};
use tokio::{
    net::TcpStream,
    process::{Child, Command},
    sync::Mutex as AsyncMutex,
};

struct Core {
    child: AsyncMutex<Option<Child>>,
    binary: PathBuf,
    assets: PathBuf,
}
#[derive(Serialize, Deserialize, Clone)]
struct Profile {
    id: String,
    name: String,
    kind: String,
    uri: String,
    source: String,
    latency: Option<u32>,
}
#[derive(Serialize, Deserialize, Default)]
struct Store {
    profiles: Vec<Profile>,
    subs: Vec<Subscription>,
}
#[derive(Serialize, Deserialize, Clone)]
struct Subscription {
    id: String,
    name: String,
    url: String,
}

fn data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("xray-glass")
}
fn store_path() -> PathBuf {
    data_dir().join("store.json")
}
fn read_store() -> Store {
    fs::read(store_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn write_store(s: &Store) -> Result<(), String> {
    fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    fs::write(
        store_path(),
        serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn decode64(s: &str) -> Option<String> {
    STANDARD
        .decode(s)
        .or_else(|_| URL_SAFE.decode(s))
        .or_else(|_| URL_SAFE_NO_PAD.decode(s))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
}
fn profile(uri: &str, source: &str, name: Option<&str>) -> Profile {
    let u = uri.trim();
    let scheme = if u.trim_start().starts_with('{') {
        serde_json::from_str::<Value>(u)
            .ok()
            .and_then(|v| {
                v.get("protocol")
                    .and_then(Value::as_str)
                    .map(str::to_lowercase)
            })
            .unwrap_or_else(|| "json".into())
    } else {
        u.split_once("://")
            .map(|x| x.0.to_lowercase())
            .unwrap_or_else(|| "json".into())
    };
    let label = name
        .map(str::to_string)
        .or_else(|| u.split('#').nth(1).map(|s| url_decode(s)))
        .unwrap_or_else(|| scheme.to_uppercase());
    Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name: label,
        kind: scheme,
        uri: u.into(),
        source: source.into(),
        latency: None,
    }
}
fn url_decode(s: &str) -> String {
    s.replace("%20", " ")
        .replace("%40", "@")
        .replace("%2F", "/")
}
fn profiles_from_text(text: &str, source: &str) -> Vec<Profile> {
    let raw = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        if let Some(arr) = v.get("outbounds").and_then(Value::as_array) {
            return arr
                .iter()
                .map(|x| profile(&x.to_string(), source, x.get("tag").and_then(Value::as_str)))
                .collect();
        }
        return vec![profile(raw, source, None)];
    }
    let expanded = if raw.contains("://") {
        raw.to_string()
    } else {
        decode64(raw).unwrap_or_else(|| raw.to_string())
    };
    expanded
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("://"))
        .map(|l| profile(l, source, None))
        .collect()
}
fn stream_settings(
    transport: &str,
    security: &str,
    path: &str,
    host: &str,
    sni: &str,
    fingerprint: &str,
    public_key: &str,
    short_id: &str,
) -> Result<Value, String> {
    let method = match transport {
        "tcp" | "raw" => "raw",
        "ws" | "websocket" => "websocket",
        "grpc" => "grpc",
        "xhttp" => "xhttp",
        "httpupgrade" => "httpupgrade",
        "mkcp" | "kcp" => "mkcp",
        "hysteria" => "hysteria",
        other => return Err(format!("Unsupported transport: {other}")),
    };
    let mut stream = json!({"method":method});
    match method {
        "websocket" => stream["wsSettings"] = json!({"path":path,"headers":{"Host":host}}),
        "grpc" => stream["grpcSettings"] = json!({"serviceName":path,"authority":host}),
        "xhttp" => stream["xhttpSettings"] = json!({"path":path,"host":host}),
        "httpupgrade" => stream["httpupgradeSettings"] = json!({"path":path,"host":host}),
        _ => {}
    }
    if security == "tls" {
        stream["security"] = json!("tls");
        stream["tlsSettings"] =
            json!({"serverName":if sni.is_empty(){host}else{sni},"allowInsecure":false});
    } else if security == "reality" {
        stream["security"] = json!("reality");
        stream["realitySettings"] = json!({"serverName":sni,"fingerprint":if fingerprint.is_empty(){"chrome"}else{fingerprint},"publicKey":public_key,"shortId":short_id});
    }
    Ok(stream)
}
fn outbound(p: &Profile) -> Result<Value, String> {
    if p.uri.trim_start().starts_with('{') {
        let v: Value = serde_json::from_str(&p.uri).map_err(|e| e.to_string())?;
        if v.get("protocol").is_some() {
            return Ok(v);
        }
    }
    if p.uri.starts_with("vmess://") {
        let body = p
            .uri
            .trim_start_matches("vmess://")
            .split('#')
            .next()
            .unwrap_or("");
        let raw = decode64(body).ok_or("Invalid VMess base64")?;
        let v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let port = v["port"]
            .as_str()
            .and_then(|x| x.parse::<u16>().ok())
            .or_else(|| v["port"].as_u64().map(|x| x as u16))
            .ok_or("Invalid VMess port")?;
        let mut o = json!({"protocol":"vmess","tag":p.name,"settings":{"address":v["add"],"port":port,"id":v["id"],"security":v["scy"].as_str().unwrap_or("auto")} });
        o["streamSettings"] = stream_settings(
            v["net"].as_str().unwrap_or("tcp"),
            v["tls"].as_str().unwrap_or(""),
            v["path"].as_str().unwrap_or(""),
            v["host"].as_str().unwrap_or(""),
            v["sni"].as_str().unwrap_or(v["add"].as_str().unwrap_or("")),
            "",
            "",
            "",
        )?;
        return Ok(o);
    }
    if p.uri.starts_with("ss://") {
        let rest = p.uri.trim_start_matches("ss://");
        let (authhost, fragment) = rest.split_once('#').unwrap_or((rest, ""));
        let authhost = authhost.split('?').next().unwrap_or(authhost);
        let decoded = decode64(authhost).unwrap_or_else(|| authhost.to_string());
        let (auth, hostport) = decoded.rsplit_once('@').ok_or("Invalid Shadowsocks link")?;
        let auth = decode64(auth).unwrap_or_else(|| auth.to_string());
        let (method, password) = auth
            .split_once(':')
            .ok_or("Invalid Shadowsocks credentials")?;
        let (host, port) = hostport.rsplit_once(':').ok_or("Missing port")?;
        let port: u16 = port
            .trim_end_matches('/')
            .parse()
            .map_err(|_| "Invalid port")?;
        return Ok(
            json!({"protocol":"shadowsocks","tag":if fragment.is_empty(){p.name.clone()}else{url_decode(fragment)},"settings":{"address":host,"port":port,"method":method,"password":password}}),
        );
    }
    let (scheme, rest) = p.uri.split_once("://").ok_or("URI not supported")?;
    let (authority, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let tag = if fragment.is_empty() {
        p.name.clone()
    } else {
        url_decode(fragment)
    };
    let clean_authority = authority.split('?').next().unwrap_or(authority);
    let (user, hostport) = clean_authority
        .rsplit_once('@')
        .unwrap_or(("", clean_authority));
    let (host, port) = hostport
        .trim_matches('/')
        .rsplit_once(':')
        .ok_or("Missing port")?;
    let port: u16 = port
        .trim_end_matches('/')
        .parse()
        .map_err(|_| "Invalid port")?;
    let s = scheme.to_lowercase();
    let (protocol, settings) = match s.as_str() {
        "vless" => {
            let id = user.split('?').next().unwrap_or(user);
            (
                "vless",
                json!({"address":host,"port":port,"id":id,"encryption":query(p.uri.as_str(),"encryption").unwrap_or_else(||"none".into()),"flow":query(p.uri.as_str(),"flow").unwrap_or_default()}),
            )
        }
        "vmess" => return Err("Invalid VMess link".into()),
        "trojan" => (
            "trojan",
            json!({"address":host,"port":port,"password":user.split('?').next().unwrap_or(user)}),
        ),
        "ss" | "shadowsocks" => {
            let auth = decode64(user).unwrap_or_else(|| user.to_string());
            let (method, password) = auth
                .split_once(':')
                .ok_or("Invalid Shadowsocks credentials")?;
            (
                "shadowsocks",
                json!({"address":host,"port":port,"method":method,"password":password}),
            )
        }
        "socks" | "socks5" => {
            let mut settings = json!({"address":host,"port":port});
            if !user.is_empty() {
                let (u, pass) = user.split_once(':').unwrap_or((user, ""));
                settings["user"] = json!(u);
                settings["pass"] = json!(pass);
            }
            ("socks", settings)
        }
        "http" => {
            let mut settings = json!({"address":host,"port":port});
            if !user.is_empty() {
                let (u, pass) = user.split_once(':').unwrap_or((user, ""));
                settings["user"] = json!(u);
                settings["pass"] = json!(pass);
            }
            ("http", settings)
        }
        "hy2" | "hysteria2" | "hysteria" => (
            "hysteria",
            json!({"version":2,"address":host,"port":port,"auth":user.split('?').next().unwrap_or(user)}),
        ),
        _ => {
            return Err(format!(
                "{} URI can be saved; Xray support requires JSON or a compatible URI",
                s.to_uppercase()
            ))
        }
    };
    let mut o = json!({"protocol":protocol,"settings":settings,"tag":tag});
    let querystr = p.uri.split('?').nth(1).unwrap_or("");
    let net = query(querystr, "type").unwrap_or_else(|| "tcp".into());
    let mut security = query(querystr, "security").unwrap_or_default();
    if security.is_empty() && matches!(s.as_str(), "trojan" | "hy2" | "hysteria2" | "hysteria") {
        security = "tls".into();
    }
    let mut stream = stream_settings(
        &net,
        &security,
        &query(querystr, "path").unwrap_or_default(),
        &query(querystr, "host").unwrap_or_default(),
        &query(querystr, "sni").unwrap_or_default(),
        &query(querystr, "fp").unwrap_or_default(),
        &query(querystr, "pbk").unwrap_or_default(),
        &query(querystr, "sid").unwrap_or_default(),
    )?;
    if matches!(s.as_str(), "hy2" | "hysteria2" | "hysteria") {
        stream["method"] = json!("hysteria");
        stream["security"] = json!("tls");
        stream["tlsSettings"] = json!({"serverName":query(querystr,"sni").unwrap_or_else(||host.to_string()),"allowInsecure":query(querystr,"insecure").as_deref()==Some("1")});
        stream["hysteriaSettings"] =
            json!({"version":2,"auth":user.split('?').next().unwrap_or(user)});
    }
    o["streamSettings"] = stream;
    Ok(o)
}
fn query(s: &str, k: &str) -> Option<String> {
    s.split('?').last()?.split('&').find_map(|x| {
        let (a, b) = x.split_once('=')?;
        if a == k {
            Some(url_decode(b))
        } else {
            None
        }
    })
}
fn endpoint(p: &Profile) -> Option<(String, u16)> {
    if p.uri.starts_with("vmess://") {
        let data = p.uri.trim_start_matches("vmess://").split('#').next()?;
        let v: Value = serde_json::from_str(&decode64(data)?).ok()?;
        let host = v["add"].as_str()?.to_string();
        let port = v["port"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| v["port"].as_u64().map(|n| n as u16))?;
        return Some((host, port));
    }
    if p.uri.trim_start().starts_with('{') {
        let v: Value = serde_json::from_str(&p.uri).ok()?;
        let settings = &v["settings"];
        let direct = settings["address"]
            .as_str()
            .map(str::to_string)
            .zip(settings["port"].as_u64().map(|n| n as u16));
        if direct.is_some() {
            return direct;
        }
        if let Some(peer) = settings["peers"].as_array().and_then(|peers| peers.first()) {
            let (host, port) = peer["endpoint"].as_str()?.rsplit_once(':')?;
            return Some((
                host.trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_string(),
                port.parse().ok()?,
            ));
        }
        for key in ["vnext", "servers"] {
            if let Some(server) = settings[key].as_array().and_then(|servers| servers.first()) {
                let host = server["address"].as_str()?.to_string();
                let port = server["port"].as_u64()? as u16;
                return Some((host, port));
            }
        }
        return None;
    }
    let mut address = p.uri.split_once("://")?.1.split('#').next()?.to_string();
    if p.uri.starts_with("ss://") {
        address = decode64(&address).unwrap_or(address);
    }
    let hostport = address.rsplit_once('@').map(|(_, h)| h).unwrap_or(&address);
    let (host, port) = hostport.trim_matches('/').rsplit_once(':')?;
    Some((host.to_string(), port.split('?').next()?.parse().ok()?))
}

#[tauri::command]
fn list_data() -> Store {
    read_store()
}
#[tauri::command]
fn import_text(text: String) -> Result<Vec<Profile>, String> {
    let mut s = read_store();
    let ps = profiles_from_text(&text, "local");
    if ps.is_empty() {
        return Err("No compatible links".into());
    }
    let mut added = Vec::new();
    for p in ps {
        if !s.profiles.iter().any(|existing| existing.uri == p.uri) {
            s.profiles.push(p.clone());
            added.push(p);
        }
    }
    write_store(&s)?;
    Ok(added)
}
#[tauri::command]
fn save_profile(name: String, kind: String, uri: String) -> Result<Profile, String> {
    let mut s = read_store();
    let mut p = profile(&uri, "manual", Some(&name));
    p.kind = kind;
    s.profiles.push(p.clone());
    write_store(&s)?;
    Ok(p)
}
#[tauri::command]
fn update_profile(id: String, name: String, kind: String, uri: String) -> Result<Profile, String> {
    let mut s = read_store();
    let p = s
        .profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or("Config not found")?;
    p.name = name;
    p.kind = kind;
    p.uri = uri;
    let updated = p.clone();
    write_store(&s)?;
    Ok(updated)
}
#[tauri::command]
fn delete_profile(id: String) -> Result<(), String> {
    let mut s = read_store();
    s.profiles.retain(|p| p.id != id);
    write_store(&s)
}
#[tauri::command]
fn clear_all() -> Result<(), String> {
    write_store(&Store::default())
}
#[tauri::command]
async fn add_subscription(name: String, url: String) -> Result<Subscription, String> {
    let sub = Subscription {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
    };
    let mut s = read_store();
    s.subs.push(sub.clone());
    write_store(&s)?;
    Ok(sub)
}
#[tauri::command]
async fn refresh_subscriptions() -> Result<usize, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let s = read_store();
    let mut found = Vec::new();
    for sub in &s.subs {
        let body = client
            .get(&sub.url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        found.extend(profiles_from_text(&body, &sub.id));
    }
    let n = found.len();
    let mut s = read_store();
    for sub in &s.subs {
        s.profiles.retain(|old| old.source != sub.id);
    }
    s.profiles.extend(found);
    write_store(&s)?;
    Ok(n)
}
#[tauri::command]
async fn start_core(
    id: String,
    mode: String,
    dns: String,
    route: String,
    core: State<'_, Core>,
) -> Result<(), String> {
    let mut c = core.child.lock().await;
    if c.as_mut()
        .is_some_and(|child| child.try_wait().ok().flatten().is_none())
    {
        return Err("Already connected".into());
    }
    *c = None;
    let s = read_store();
    let p = s
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or("Config not found")?;
    let ob = outbound(p)?;
    let mut inbounds = vec![
        json!({"listen":"127.0.0.1","port":10808,"protocol":"socks","tag":"socks-in","settings":{"udp":true}}),
        json!({"listen":"127.0.0.1","port":10809,"protocol":"http","tag":"http-in"}),
    ];
    if mode == "tun" {
        inbounds.insert(0,json!({"protocol":"tun","tag":"tun-in","settings":{"name":"xray0","mtu":1500,"gateway":["10.88.0.1/30"],"autoSystemRoutingTable":["0.0.0.0/0","::/0"],"autoOutboundsInterface":"auto"}}));
    }
    let mut outs = vec![ob];
    outs.push(json!({"protocol":"freedom","tag":"direct"}));
    let mut rules = vec![json!({"type":"field","ip":["geoip:private"],"outboundTag":"direct"})];
    if route == "ir" {
        rules.insert(
            0,
            json!({"type":"field","ip":["geoip:ir"],"outboundTag":"direct"}),
        );
        rules.insert(0,json!({"type":"field","domain":["regexp:.*\\.ir$","regexp:.*\\.xn--mgba3a4f16a$"] ,"outboundTag":"direct"}));
    }
    let conf = json!({"log":{"loglevel":"warning"},"dns":{"servers":[if dns.is_empty(){"https://1.1.1.1/dns-query"}else{&dns}]},"inbounds":inbounds,"outbounds":outs,"routing":{"domainStrategy":"IPIfNonMatch","rules":rules}});
    fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    let path = data_dir().join("active.json");
    fs::write(&path, serde_json::to_vec_pretty(&conf).unwrap()).map_err(|e| e.to_string())?;
    let bin = &core.binary;
    let test = Command::new(bin)
        .env("XRAY_LOCATION_ASSET", &core.assets)
        .args(["run", "-test", "-c"])
        .arg(&path)
        .output()
        .await
        .map_err(|e| format!("Xray unavailable: {e}"))?;
    if !test.status.success() {
        return Err(String::from_utf8_lossy(&test.stderr).to_string());
    }
    let mut command = if mode == "tun" {
        let mut elevated = Command::new("pkexec");
        elevated
            .arg("env")
            .arg(format!("XRAY_LOCATION_ASSET={}", core.assets.display()))
            .arg(bin);
        elevated
    } else {
        let mut ordinary = Command::new(bin);
        ordinary.env("XRAY_LOCATION_ASSET", &core.assets);
        ordinary
    };
    let child = command
        .args(["run", "-c"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            if mode == "tun" {
                format!("TUN needs pkexec and a PolicyKit authentication agent: {e}")
            } else {
                e.to_string()
            }
        })?;
    *c = Some(child);
    Ok(())
}
#[tauri::command]
async fn stop_core(core: State<'_, Core>) -> Result<(), String> {
    let mut c = core.child.lock().await;
    if let Some(mut p) = c.take() {
        p.kill().await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
async fn core_running(core: State<'_, Core>) -> bool {
    let mut child = core.child.lock().await;
    let running = child.as_mut().is_some_and(|process| {
        process
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    });
    if !running {
        *child = None;
    }
    running
}
#[tauri::command]
async fn core_version(core: State<'_, Core>) -> String {
    Command::new(&core.binary)
        .arg("version")
        .output()
        .await
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}
#[tauri::command]
async fn check_update() -> Result<String, String> {
    let v = reqwest::get("https://api.github.com/repos/XTLS/Xray-core/releases/latest")
        .await
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(v["tag_name"].as_str().unwrap_or("—").to_string())
}
#[tauri::command]
async fn ping_all() -> Vec<(String, Option<u32>)> {
    let s = read_store();
    let mut out = Vec::new();
    for p in s.profiles {
        let ms = if let Some((host, port)) = endpoint(&p) {
            let t = std::time::Instant::now();
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                TcpStream::connect((host.as_str(), port)),
            )
            .await
            .ok()
            .and_then(Result::ok)
            .map(|_| t.elapsed().as_millis() as u32)
        } else {
            None
        };
        out.push((p.id, ms));
    }
    out
}
#[tauri::command]
async fn speed_test(core: State<'_, Core>) -> Result<u64, String> {
    let t = std::time::Instant::now();
    let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
    if let Some(child) = core.child.lock().await.as_mut() {
        if child.try_wait().ok().flatten().is_none() {
            builder = builder.proxy(
                reqwest::Proxy::all("socks5h://127.0.0.1:10808").map_err(|e| e.to_string())?,
            );
        }
    }
    let client = builder.build().map_err(|e| e.to_string())?;
    let b = client
        .get("https://speed.cloudflare.com/__down?bytes=5000000")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let sec = t.elapsed().as_secs_f64().max(0.1);
    Ok(((b.len() as f64 * 8.0 / sec) / 1_000_000.0) as u64)
}
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let resource = app.path().resource_dir().unwrap_or_else(|_| data_dir());
            let local_binary = std::env::current_dir()
                .unwrap_or_default()
                .join("src-tauri/resources/xray");
            let binary = [resource.join("xray"), local_binary, PathBuf::from("xray")]
                .into_iter()
                .find(|path| path.is_file())
                .unwrap_or_else(|| PathBuf::from("xray"));
            app.manage(Core {
                child: AsyncMutex::new(None),
                binary,
                assets: resource,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_data,
            import_text,
            save_profile,
            update_profile,
            delete_profile,
            clear_all,
            add_subscription,
            refresh_subscriptions,
            start_core,
            stop_core,
            core_running,
            core_version,
            check_update,
            ping_all,
            speed_test
        ])
        .run(tauri::generate_context!())
        .expect("error while running app");
}
