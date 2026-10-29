use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::PathBuf, process::Stdio};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, BufReader},
    net::TcpListener,
    process::{Child, Command},
    sync::Mutex as AsyncMutex,
};

fn stream_logs<R: AsyncBufRead + Unpin + Send + 'static>(
    reader: R,
    app: AppHandle,
    source: &'static str,
    level: &'static str,
) {
    tokio::spawn(async move {
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_owned();
            if line.is_empty() {
                continue;
            }
            let _ = app.emit("core-log", json!({"time": chrono_placeholder(), "source": source, "level": level, "message": line}));
        }
    });
}
fn chrono_placeholder() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_default()
}

struct Core {
    child: AsyncMutex<Option<Child>>,
    aether_child: AsyncMutex<Option<Child>>,
    mhr_child: AsyncMutex<Option<Child>>,
    sni_child: AsyncMutex<Option<Child>>,
    binary: PathBuf,
    aether_binary: PathBuf,
    singbox_binary: PathBuf,
    mihomo_binary: PathBuf,
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
    #[serde(default)]
    user_agent: String,
    #[serde(default)]
    more_urls: String,
}

fn data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("xray-glass")
}
fn store_path() -> PathBuf {
    data_dir().join("store.json")
}
fn secrets_path() -> PathBuf {
    data_dir().join("client-secrets.json")
}
fn read_client_secrets() -> Value {
    fs::read(secrets_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}))
}
#[tauri::command]
fn client_secrets() -> Value {
    read_client_secrets()
}
#[tauri::command]
fn save_client_secrets(values: Value) -> Result<(), String> {
    let mut saved = read_client_secrets();
    let map = saved
        .as_object_mut()
        .ok_or("Invalid secure settings file")?;
    if let Some(incoming) = values.as_object() {
        for (key, value) in incoming {
            if matches!(
                key.as_str(),
                "mhr_script_id"
                    | "mhr_auth_key"
                    | "aether_access_email"
                    | "aether_access_id"
                    | "aether_access_secret"
                    | "aether_access_token"
            ) {
                map.insert(key.clone(), value.clone());
            }
        }
    }
    fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    let path = secrets_path();
    fs::write(
        &path,
        serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    Ok(())
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
fn build_client_config(
    profile: &Profile,
    mode: &str,
    dns: &str,
    dns_enabled: bool,
    route: &str,
    share_lan: bool,
) -> Result<Value, String> {
    let listen = if share_lan { "0.0.0.0" } else { "127.0.0.1" };
    let mut inbounds = vec![
        json!({"listen":listen,"port":10808,"protocol":"socks","tag":"socks-in","settings":{"udp":true}}),
        json!({"listen":listen,"port":10809,"protocol":"http","tag":"http-in"}),
    ];
    if mode == "tun" {
        let ipv6_enabled = fs::read_to_string("/proc/sys/net/ipv6/conf/all/disable_ipv6")
            .map(|value| value.trim() != "1")
            .unwrap_or(false);
        let (routes, gateway) = if ipv6_enabled {
            (
                json!(["0.0.0.0/0", "::/0"]),
                json!(["10.88.0.1/30", "fdfe:dcba:9876::1/126"]),
            )
        } else {
            (json!(["0.0.0.0/0"]), json!(["10.88.0.1/30"]))
        };
        inbounds.insert(0,json!({"protocol":"tun","tag":"tun","settings":{"name":"xray_tun","mtu":9000,"gateway":gateway,"autoSystemRoutingTable":routes,"autoOutboundsInterface":"auto","autoSystemDnsToGateway":true},"sniffing":{"enabled":true,"destOverride":["http","tls"]}}));
    }
    let mut rules = vec![json!({"type":"field","ip":["geoip:private"],"outboundTag":"direct"})];
    if mode == "tun" {
        rules.insert(
            0,
            json!({"type":"field","ip":["224.0.0.0/3","ff00::/8"],"outboundTag":"block"}),
        );
        rules.insert(
            0,
            json!({"type":"field","network":"udp","port":"135,137-139,5353","outboundTag":"block"}),
        );
        if dns_enabled {
            rules.insert(
                0,
                json!({"type":"field","inboundTag":["tun"],"port":"53","outboundTag":"dns"}),
            );
        }
    }
    match route {
        "ir" => {
            rules.insert(
                0,
                json!({"type":"field","ip":["geoip:ir"],"outboundTag":"direct"}),
            );
            rules.insert(0, json!({"type":"field","domain":["regexp:.*\\.ir$","regexp:.*\\.xn--mgba3a4f16a$"],"outboundTag":"direct"}));
        }
        "direct" => rules.insert(
            if mode == "tun" { 3 } else { 0 },
            json!({"type":"field","network":"tcp,udp","outboundTag":"direct"}),
        ),
        _ => {}
    }
    let mut config = json!({
        "log":{"loglevel":"warning"},
        "dns":{"servers":[if dns.is_empty(){"https://1.1.1.1/dns-query"}else{dns}]},
        "inbounds":inbounds,
        "outbounds":[outbound(profile)?,{"protocol":"freedom","tag":"direct"},{"protocol":"blackhole","tag":"block"},{"protocol":"dns","tag":"dns","settings":{"userLevel":12}}],
        "routing":{"domainStrategy":"IPIfNonMatch","rules":rules}
    });
    if !dns_enabled {
        config.as_object_mut().unwrap().remove("dns");
    }
    Ok(config)
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
        .or_else(|| {
            if matches!(scheme.as_str(), "sing-box" | "mihomo") {
                None
            } else {
                u.split('#').nth(1).map(url_decode)
            }
        })
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
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let input = s.as_bytes();
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' && index + 2 < input.len() {
            if let (Some(high), Some(low)) = (hex(input[index + 1]), hex(input[index + 2])) {
                output.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        output.push(if input[index] == b'+' {
            b' '
        } else {
            input[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}
fn parse_wireguard_config(raw: &str, source: &str) -> Vec<Profile> {
    let mut interface = HashMap::new();
    let mut peers: Vec<HashMap<String, String>> = Vec::new();
    let mut section = "";
    for line in raw.lines().map(str::trim) {
        if line.eq_ignore_ascii_case("[interface]") {
            section = "interface";
            continue;
        }
        if line.eq_ignore_ascii_case("[peer]") {
            section = "peer";
            peers.push(HashMap::new());
            continue;
        }
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_lowercase();
            let value = value
                .split(['#', ';'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if section == "interface" {
                interface.insert(key, value);
            } else if section == "peer" {
                if let Some(peer) = peers.last_mut() {
                    peer.insert(key, value);
                }
            }
        }
    }
    let Some(private_key) = interface.get("privatekey").filter(|key| !key.is_empty()) else {
        return Vec::new();
    };
    let split = |value: Option<&String>| -> Vec<String> {
        value
            .into_iter()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect()
    };
    let addresses = {
        let parsed = split(interface.get("address"));
        if parsed.is_empty() {
            vec!["172.16.0.2/32".to_string()]
        } else {
            parsed
        }
    };
    let remote_dns = split(interface.get("dns"));
    let mtu = interface
        .get("mtu")
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1420);
    peers
        .iter()
        .enumerate()
        .filter_map(|(index, peer)| {
            let endpoint = peer.get("endpoint")?;
            let (host, port) = if endpoint.starts_with('[') {
                let close = endpoint.find(']')?;
                let host = &endpoint[1..close];
                let port = endpoint[close + 1..]
                    .strip_prefix(':')
                    .unwrap_or("2408")
                    .parse::<u16>()
                    .ok()?;
                (host, port)
            } else if let Some((host, port)) = endpoint.rsplit_once(':') {
                (host, port.parse::<u16>().ok()?)
            } else {
                (endpoint.as_str(), 2408)
            };
            let mut peer_json = json!({
                "publicKey":peer.get("publickey").map(String::as_str).unwrap_or(""),
                "endpoint":format!("{}:{}", if host.contains(':') { format!("[{host}]") } else { host.to_string() }, port),
                "allowedIPs":split(peer.get("allowedips"))
            });
            if let Some(key) = peer.get("presharedkey") {
                peer_json["preSharedKey"] = json!(key);
            }
            let mut settings = json!({
                "secretKey":private_key,
                "address":addresses,
                "mtu":mtu,
                "remoteDNS":remote_dns,
                "peers":[peer_json]
            });
            if let Some(reserved) = peer.get("reserved") {
                let values: Vec<u8> = split(Some(reserved))
                    .iter()
                    .filter_map(|value| value.parse().ok())
                    .collect();
                if !values.is_empty() {
                    settings["reserved"] = json!(values);
                }
            }
            let uri = json!({"protocol":"wireguard","tag":format!("WireGuard Peer {}",index+1),"settings":settings}).to_string();
            Some(profile(&uri, source, Some(&format!("WireGuard Peer {}", index + 1))))
        })
        .collect()
}
fn profiles_from_text(text: &str, source: &str) -> Vec<Profile> {
    let raw = text.trim();
    if raw.starts_with("sing-box://") || raw.starts_with("mihomo://") {
        let scheme = raw.split_once("://").map(|(s, _)| s).unwrap_or("engine");
        return vec![profile(raw, source, Some(&format!("{} config", scheme)))];
    }
    if raw
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("[Interface]"))
    {
        return parse_wireguard_config(raw, source);
    }
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        if let Some(entries) = v.as_array() {
            let profiles = entries
                .iter()
                .filter_map(|entry| {
                    let host = entry.get("server")?.as_str()?;
                    let port = entry.get("server_port")?.as_u64()?;
                    let method = entry.get("method")?.as_str()?;
                    let password = entry.get("password")?.as_str()?;
                    let name = entry
                        .get("remarks")
                        .or_else(|| entry.get("name"))
                        .and_then(Value::as_str)
                        .filter(|name| !name.is_empty())
                        .unwrap_or("Shadowsocks");
                    let uri = json!({
                        "protocol":"shadowsocks",
                        "tag":name,
                        "settings":{"address":host,"port":port,"method":method,"password":password}
                    })
                    .to_string();
                    Some(profile(&uri, source, Some(name)))
                })
                .collect::<Vec<_>>();
            return profiles;
        }
        if let Some(arr) = v.get("outbounds").and_then(Value::as_array) {
            return arr
                .iter()
                .filter(|outbound| {
                    !matches!(
                        outbound.get("protocol").and_then(Value::as_str),
                        Some("freedom" | "blackhole" | "dns" | "loopback")
                    )
                })
                .map(|x| profile(&x.to_string(), source, x.get("tag").and_then(Value::as_str)))
                .collect();
        }
        if matches!(
            v.get("protocol").and_then(Value::as_str),
            Some("freedom" | "blackhole" | "dns" | "loopback")
        ) {
            return Vec::new();
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
    header_type: &str,
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
        "raw" if header_type == "http" => {
            stream["rawSettings"] = json!({"header":{"type":"http","request":{},"response":{}}});
        }
        "websocket" => {
            stream["wsSettings"] = json!({"path":path,"host":host,"headers":{"Host":host}})
        }
        "grpc" => stream["grpcSettings"] = json!({"serviceName":path,"authority":host}),
        "xhttp" => stream["xhttpSettings"] = json!({"path":path,"host":host}),
        "httpupgrade" => stream["httpupgradeSettings"] = json!({"path":path,"host":host}),
        "mkcp" => {
            stream["kcpSettings"] = json!({"seed":path,"header":{"type":if header_type.is_empty(){"none"}else{header_type}}})
        }
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
    if p.kind.eq_ignore_ascii_case("aether") {
        return Ok(
            json!({"protocol":"socks","tag":p.name,"settings":{"servers":[{"address":"127.0.0.1","port":1819}]}}),
        );
    }
    if p.kind.eq_ignore_ascii_case("mhr") {
        return Ok(
            json!({"protocol":"socks","tag":p.name,"settings":{"servers":[{"address":"127.0.0.1","port":18080}]}}),
        );
    }
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
            v["type"].as_str().unwrap_or("none"),
            v["tls"].as_str().unwrap_or(""),
            v["path"].as_str().unwrap_or(""),
            v["host"].as_str().unwrap_or(""),
            v["sni"].as_str().unwrap_or(v["add"].as_str().unwrap_or("")),
            v["fp"].as_str().unwrap_or(""),
            v["pbk"].as_str().unwrap_or(""),
            v["sid"].as_str().unwrap_or(""),
        )?;
        if let Some(alpn) = v["alpn"].as_str().filter(|value| !value.is_empty()) {
            o["streamSettings"]["tlsSettings"]["alpn"] = json!(alpn.split(',').collect::<Vec<_>>());
        }
        if let Some(spider_x) = v["spx"].as_str() {
            o["streamSettings"]["realitySettings"]["spiderX"] = json!(spider_x);
        }
        if v["insecure"].as_str() == Some("1") || v["allowInsecure"].as_str() == Some("1") {
            o["streamSettings"]["tlsSettings"]["allowInsecure"] = json!(true);
        }
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
            let id = url_decode(user.split('?').next().unwrap_or(user));
            (
                "vless",
                json!({"address":host,"port":port,"id":id,"encryption":query(p.uri.as_str(),"encryption").unwrap_or_else(||"none".into()),"flow":query(p.uri.as_str(),"flow").unwrap_or_default()}),
            )
        }
        "vmess" => return Err("Invalid VMess link".into()),
        "trojan" => (
            "trojan",
            json!({"address":host,"port":port,"password":url_decode(user.split('?').next().unwrap_or(user))}),
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
        "hy2" | "hysteria2" | "hysteria" | "tuic" | "wireguard" => {
            return Err(format!(
                "{} profiles use the sing-box engine. Add a complete sing-box JSON profile in Settings → Engine Profiles to run it.",
                s.to_uppercase()
            ));
        }
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
    if security.is_empty() && s == "trojan" {
        security = "tls".into();
    }
    let mut stream = stream_settings(
        &net,
        &query(querystr, "headerType").unwrap_or_else(|| "none".into()),
        &security,
        &query(querystr, "path").unwrap_or_default(),
        &query(querystr, "host").unwrap_or_default(),
        &query(querystr, "sni").unwrap_or_default(),
        &query(querystr, "fp").unwrap_or_default(),
        &query(querystr, "pbk").unwrap_or_default(),
        &query(querystr, "sid").unwrap_or_default(),
    )?;
    if security == "tls"
        && (query(querystr, "insecure").as_deref() == Some("1")
            || query(querystr, "allowInsecure").as_deref() == Some("1"))
    {
        stream["tlsSettings"]["allowInsecure"] = json!(true);
    }
    if let Some(alpn) = query(querystr, "alpn").filter(|value| !value.is_empty()) {
        stream["tlsSettings"]["alpn"] = json!(alpn.split(',').collect::<Vec<_>>());
    }
    if let Some(spider_x) = query(querystr, "spx") {
        stream["realitySettings"]["spiderX"] = json!(spider_x);
    }
    o["streamSettings"] = stream;
    Ok(o)
}
fn singbox_link_config(
    profile: &Profile,
    mode: &str,
    share_lan: bool,
    route_mode: &str,
) -> Result<Value, String> {
    let uri = profile.uri.as_str();
    let (scheme, rest) = uri.split_once("://").ok_or("Invalid proxy URI")?;
    let clean = rest.split(['?', '#']).next().unwrap_or(rest);
    let authority = clean
        .split_once('/')
        .map(|(authority, _)| authority)
        .unwrap_or(clean);
    let (credentials, hostport) = authority
        .rsplit_once('@')
        .ok_or("Proxy URI must include credentials")?;
    let (host, port) = if hostport.starts_with('[') {
        let end = hostport.find(']').ok_or("Invalid IPv6 host")?;
        (
            &hostport[1..end],
            hostport[end + 1..]
                .strip_prefix(':')
                .ok_or("Missing port")?,
        )
    } else {
        hostport.rsplit_once(':').ok_or("Missing port")?
    };
    let port: u16 = port.parse().map_err(|_| "Invalid port")?;
    let server_name = query(uri, "sni")
        .or_else(|| query(uri, "serverName"))
        .unwrap_or_else(|| host.to_owned());
    let insecure = matches!(query(uri, "allowInsecure").as_deref(), Some("1" | "true"));
    let mut outbound = match scheme.to_ascii_lowercase().as_str() {
        "hysteria2" | "hy2" => {
            let mut value = json!({"type":"hysteria2","tag":"proxy","server":host,"server_port":port,"password":url_decode(credentials),"tls":{"enabled":true,"server_name":server_name,"insecure":insecure}});
            if let Some(obfs) = query(uri, "obfs").filter(|v| !v.is_empty()) {
                value["obfs"] =
                    json!({"type":obfs,"password":query(uri,"obfs-password").unwrap_or_default()});
            }
            for key in ["up_mbps", "down_mbps"] {
                if let Some(n) = query(uri, key).and_then(|v| v.parse::<u64>().ok()) {
                    value[key] = json!(n);
                }
            }
            value
        }
        "tuic" => {
            let (uuid, password) = credentials.split_once(':').unwrap_or((credentials, ""));
            json!({"type":"tuic","tag":"proxy","server":host,"server_port":port,"uuid":url_decode(uuid),"password":url_decode(password),"congestion_control":query(uri,"congestion_control").unwrap_or_else(||"cubic".into()),"udp_relay_mode":query(uri,"udp_relay_mode").unwrap_or_else(||"native".into()),"tls":{"enabled":true,"server_name":server_name,"insecure":insecure}})
        }
        other => return Err(format!("Unsupported sing-box URI: {other}")),
    };
    outbound["tag"] = json!("proxy");
    let inbound = if mode == "tun" {
        json!({"type":"tun","tag":"tun-in","interface_name":"xray_glass","address":["172.19.0.1/30","fdfe:dcba:9876::1/126"],"auto_route":true,"strict_route":true,"stack":"system"})
    } else {
        json!({"type":"mixed","tag":"mixed-in","listen":if share_lan {"0.0.0.0"} else {"127.0.0.1"},"listen_port":10808})
    };
    let mut route = json!({"final":if route_mode == "direct" {"direct"} else {"proxy"},"auto_detect_interface":true});
    if route_mode == "ir" {
        route["rules"] = json!([{"domain_suffix":[".ir",".xn--mgba3a4f16a"],"outbound":"direct"}]);
        route["final"] = json!("proxy");
    }
    Ok(
        json!({"log":{"level":"warn"},"inbounds":[inbound],"outbounds":[outbound,{"type":"direct","tag":"direct"}],"route":route}),
    )
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
fn clear_stats() -> Result<(), String> {
    let mut store = read_store();
    for profile in &mut store.profiles {
        profile.latency = None;
    }
    write_store(&store)
}
#[tauri::command]
fn export_backup() -> Result<String, String> {
    serde_json::to_string_pretty(&read_store()).map_err(|e| e.to_string())
}
#[tauri::command]
fn import_backup(text: String) -> Result<(), String> {
    let store: Store = serde_json::from_str(&text).map_err(|e| format!("Invalid backup: {e}"))?;
    if store
        .profiles
        .iter()
        .any(|profile| profile.uri.trim().is_empty())
    {
        return Err("Backup contains an empty profile".into());
    }
    write_store(&store)
}
#[tauri::command]
fn open_storage_location() -> Result<(), String> {
    fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    std::process::Command::new("xdg-open")
        .arg(data_dir())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn add_subscription(
    name: String,
    url: String,
    user_agent: Option<String>,
    more_urls: Option<String>,
) -> Result<Subscription, String> {
    let sub = Subscription {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
        user_agent: user_agent.unwrap_or_default(),
        more_urls: more_urls.unwrap_or_default(),
    };
    let mut s = read_store();
    s.subs.push(sub.clone());
    write_store(&s)?;
    Ok(sub)
}
#[tauri::command]
async fn refresh_subscriptions(mode: String, only: Option<String>) -> Result<usize, String> {
    let s = read_store();
    let subscriptions: Vec<_> = s
        .subs
        .iter()
        .filter(|sub| only.as_deref().map_or(true, |id| sub.id == id))
        .cloned()
        .collect();
    if subscriptions.is_empty() {
        return Err("No subscription selected".into());
    }
    let mut found = Vec::new();
    for sub in &subscriptions {
        let timeout = std::time::Duration::from_secs(18);
        let mut direct_builder = reqwest::Client::builder().no_proxy().timeout(timeout);
        let mut proxied_builder = reqwest::Client::builder()
            .timeout(timeout)
            .proxy(reqwest::Proxy::all("socks5h://127.0.0.1:10808").map_err(|e| e.to_string())?);
        if !sub.user_agent.trim().is_empty() {
            direct_builder = direct_builder.user_agent(&sub.user_agent);
            proxied_builder = proxied_builder.user_agent(&sub.user_agent);
        }
        let direct = direct_builder.build().map_err(|e| e.to_string())?;
        let proxied = proxied_builder.build().map_err(|e| e.to_string())?;
        let urls = std::iter::once(sub.url.trim())
            .chain(sub.more_urls.split([',', '\n']).map(str::trim))
            .filter(|url| !url.is_empty())
            .collect::<Vec<_>>();
        let fetch = |client: reqwest::Client, url: String, source: String| async move {
            let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
            if !response.status().is_success() {
                return Err(format!("HTTP {}", response.status()));
            }
            let body = response.text().await.map_err(|e| e.to_string())?;
            let profiles = profiles_from_text(&body, &source);
            if profiles.is_empty() {
                return Err("Response did not contain supported profiles".into());
            }
            Ok::<_, String>(profiles)
        };
        for url in urls {
            let parsed = match mode.as_str() {
                "direct" => fetch(direct.clone(), url.to_string(), sub.id.clone()).await,
                "proxy" => fetch(proxied.clone(), url.to_string(), sub.id.clone()).await,
                _ => {
                    let mut direct_request = Some(tokio::spawn(fetch(
                        direct.clone(),
                        url.to_string(),
                        sub.id.clone(),
                    )));
                    let mut proxy_request = Some(tokio::spawn(fetch(
                        proxied.clone(),
                        url.to_string(),
                        sub.id.clone(),
                    )));
                    loop {
                        if direct_request
                            .as_ref()
                            .is_some_and(|task| task.is_finished())
                        {
                            let first = direct_request
                                .take()
                                .unwrap()
                                .await
                                .unwrap_or_else(|error| Err(error.to_string()));
                            if first.is_ok() {
                                break first;
                            }
                        }
                        if direct_request.is_none()
                            && proxy_request
                                .as_ref()
                                .is_some_and(|task| task.is_finished())
                        {
                            break proxy_request
                                .take()
                                .unwrap()
                                .await
                                .unwrap_or_else(|error| Err(error.to_string()));
                        }
                        if direct_request.is_some()
                            && proxy_request
                                .as_ref()
                                .is_some_and(|task| task.is_finished())
                        {
                            let first = proxy_request
                                .take()
                                .unwrap()
                                .await
                                .unwrap_or_else(|error| Err(error.to_string()));
                            if first.is_ok() {
                                break first;
                            }
                            break direct_request
                                .take()
                                .unwrap()
                                .await
                                .unwrap_or_else(|error| Err(error.to_string()));
                        }
                        if direct_request.is_none() && proxy_request.is_none() {
                            break Err("Both subscription download routes failed".to_owned());
                        }
                        if direct_request.is_none() || proxy_request.is_none() {
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                            continue;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }
            };
            if let Ok(parsed) = parsed {
                found.extend(parsed);
            }
        }
        if !found
            .iter()
            .any(|profile: &Profile| profile.source == sub.id)
        {
            return Err(format!("No configs in subscription {}", sub.name));
        }
    }
    let n = found.len();
    let mut s = read_store();
    for sub in &subscriptions {
        s.profiles.retain(|old| old.source != sub.id);
    }
    s.profiles.extend(found);
    write_store(&s)?;
    Ok(n)
}
fn aether_args(profile: &Profile, options: &Value) -> Result<Vec<String>, String> {
    let option = |key: &str, default: &str| {
        query(&profile.uri, key)
            .filter(|value| !value.is_empty())
            .or_else(|| options.get(key).and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| default.to_owned())
    };
    let protocol = option("protocol", "masque");
    if !["masque", "wg", "gool", "mim", "wg-over-masque"].contains(&protocol.as_str()) {
        return Err(format!("Unsupported Aether protocol: {protocol}"));
    }
    let transport = option("transport", "h3");
    let scan = option("scan", "balanced");
    let psiphon = options
        .get("psiphon")
        .and_then(Value::as_str)
        .unwrap_or("off");
    let psiphon_mode = options
        .get("psiphon_mode")
        .and_then(Value::as_str)
        .unwrap_or("auto");
    let tor = options.get("tor").and_then(Value::as_str).unwrap_or("off");
    if !["h2", "h3"].contains(&transport.as_str())
        || !["turbo", "balanced", "thorough", "verified", "ironclad"].contains(&scan.as_str())
        || !["off", "chain", "reverse", "only"].contains(&psiphon)
        || !["auto", "cdn", "direct"].contains(&psiphon_mode)
        || !["off", "chain", "reverse", "only"].contains(&tor)
    {
        return Err("Invalid Aether transport, scan, Tor, or Psiphon setting".into());
    }
    if psiphon == "only" && tor != "off" || tor == "only" && psiphon != "off" {
        return Err("Aether cannot run Tor-only and Psiphon-only together".into());
    }
    let only_carrier = psiphon == "only" || tor == "only";
    let core_protocol = if protocol == "wg-over-masque" {
        "gool"
    } else {
        protocol.as_str()
    };
    let mut args = vec![
        "--bind".into(),
        "127.0.0.1:1819".into(),
        "--log-level".into(),
        "info".into(),
    ];
    if !only_carrier {
        let ip_value = option("ip", "dual");
        let ip = match ip_value.as_str() {
            "v4" => "4",
            "v6" => "6",
            "both" => "dual",
            other => other,
        };
        if !["4", "6", "dual"].contains(&ip) {
            return Err("Aether IP mode must be IPv4, IPv6, or dual".into());
        }
        args.extend([
            "--protocol".into(),
            core_protocol.to_owned(),
            "--scan".into(),
            scan,
            "--ip".into(),
            ip.to_owned(),
            "--quick-reconnect".into(),
        ]);
        if protocol == "gool" {
            args.extend(["--gool-classic".into(), "--wiw-scan".into()]);
        }
        if protocol == "mim" {
            args.push("--mim-scan".into());
        }
    }
    if !only_carrier && (protocol == "masque" || protocol == "mim" || protocol == "wg-over-masque")
    {
        args.extend(["--masque-sni".into(), option("sni", "www.cloudflare.com")]);
        if transport == "h2" {
            args.push("--h2".into());
            let fragment = query(&profile.uri, "fragment")
                .map(|value| value == "1")
                .unwrap_or_else(|| options.get("fragment").and_then(Value::as_bool) == Some(true));
            if fragment {
                args.push("--fragment".into());
                args.extend(["--fragment-size".into(), option("fragment_size", "100-200")]);
                args.extend(["--fragment-delay".into(), option("fragment_delay", "10-30")]);
            }
        }
        if options.get("ech").and_then(Value::as_bool) == Some(true)
            || query(&profile.uri, "ech").as_deref() == Some("1")
        {
            args.extend(["--ech".into(), "auto".into()]);
            args.extend(["--ech-dns".into(), option("ech_dns", "udp://1.1.1.1")]);
            args.extend([
                "--ech-domain".into(),
                option("ech_domain", "cloudflare-ech.com"),
            ]);
        }
    }
    let noize = option("noize", "auto");
    if !only_carrier
        && [
            "auto",
            "off",
            "light",
            "firewall",
            "balanced",
            "gfw",
            "aggressive",
        ]
        .contains(&noize.as_str())
    {
        args.extend(["--noize".into(), noize]);
    }
    let peer = query(&profile.uri, "peer").or_else(|| {
        let authority = profile
            .uri
            .split_once("://")?
            .1
            .split(['/', '?', '#'])
            .next()?;
        if authority.is_empty() {
            None
        } else {
            Some(authority.to_owned())
        }
    });
    if !only_carrier {
        if let Some(peer) = peer.filter(|value| {
            value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".:-[]".contains(&b))
        }) {
            if protocol != "gool" && protocol != "mim" && protocol != "wg-over-masque" {
                args.extend(["--peer".into(), peer]);
            }
        }
    }
    let dns = query(&profile.uri, "dns")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            options
                .get("dns")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
        });
    if !only_carrier {
        if let Some(dns) = dns.filter(|value| !value.trim().is_empty()) {
            args.extend(["--dns".into(), dns.trim().to_owned()]);
        }
    }
    if !only_carrier {
        if let Some(exit) = options
            .get("exit_loc")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            args.extend(["--exit-loc".into(), exit.trim().to_owned()]);
        }
    }
    match tor {
        "chain" => args.push("--tor".into()),
        "reverse" => args.push("--tor-reverse".into()),
        "only" => args.push("--tor-only".into()),
        _ => {}
    }
    match psiphon {
        "chain" => args.push("--psiphon".into()),
        "reverse" => args.push("--psiphon-reverse".into()),
        "only" => args.push("--psiphon-only".into()),
        _ => {}
    }
    if psiphon != "off" {
        args.extend(["--psiphon-mode".into(), psiphon_mode.into()]);
        for (key, flag) in [
            ("psiphon_region", "--psiphon-region"),
            ("psiphon_cdn_ips", "--psiphon-cdn-ips"),
            ("psiphon_cdn_sni", "--psiphon-cdn-sni"),
        ] {
            if let Some(value) = options
                .get(key)
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                args.extend([flag.into(), value.trim().into()]);
            }
        }
    }
    let team = options
        .get("team")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if !team.is_empty() {
        args.extend(["--team".into(), team.into()]);
        for (key, flag) in [
            ("access_email", "--access-email"),
            ("access_id", "--access-id"),
            ("access_secret", "--access-secret"),
            ("access_token", "--access-token"),
        ] {
            if let Some(value) = options
                .get(key)
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                args.extend([flag.into(), value.trim().into()]);
            }
        }
    }
    Ok(args)
}
#[tauri::command]
async fn aether_scan(
    id: String,
    aether_options: Value,
    core: State<'_, Core>,
) -> Result<u64, String> {
    if tokio::net::TcpStream::connect("127.0.0.1:1819")
        .await
        .is_ok()
    {
        return Err("Stop the current Aether session before scanning".into());
    }
    let profile = read_store()
        .profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("Config not found")?;
    if !profile.kind.eq_ignore_ascii_case("aether") {
        return Err("Select an Aether profile first".into());
    }
    if !core.aether_binary.is_file() {
        return Err("Aether core is missing; rebuild or install the Aether binary".into());
    }
    let mut args = aether_args(&profile, &aether_options)?;
    if let Some(at) = args.iter().position(|arg| arg == "--quick-reconnect") {
        args.remove(at);
    }
    args.push("--no-quick-reconnect".into());
    let work_dir = data_dir().join("aether");
    fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;
    let mut command = Command::new(&core.aether_binary);
    command
        .current_dir(&work_dir)
        .env("AETHER_TOR_PT_DIR", core.assets.join("pt"));
    if core.assets.join("pt/psiphon-tunnel-core").is_file() {
        command.env(
            "AETHER_PSIPHON_BIN",
            core.assets.join("pt/psiphon-tunnel-core"),
        );
    }
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not start Aether scanner: {e}"))?;
    let started = std::time::Instant::now();
    let result = async {
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::all("socks5h://127.0.0.1:1819").map_err(|e| e.to_string())?)
            .timeout(std::time::Duration::from_secs(70))
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .get("https://www.cloudflare.com/cdn-cgi/trace")
            .send()
            .await
            .map_err(|e| format!("Aether scan failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("Aether scan returned HTTP {}", response.status()));
        }
        Ok(started.elapsed().as_millis() as u64)
    }
    .await;
    let _ = child.kill().await;
    result
}
fn mhr_config(options: &Value) -> Result<Value, String> {
    let script_id = options
        .get("script_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let auth_key = options
        .get("auth_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if script_id.is_empty() || auth_key.len() < 24 || auth_key == "CHANGE_ME_TO_A_STRONG_SECRET" {
        return Err(
            "MHR needs your deployed Apps Script ID and an AUTH_KEY of at least 24 characters"
                .into(),
        );
    }
    let script_ids: Vec<&str> = script_id
        .split([',', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    Ok(json!({
        "google_ip":"216.239.38.120", "front_domain":"www.google.com", "front_domains":["www.google.com","mail.google.com","accounts.google.com"],
        "script_id":script_ids.first(), "script_ids":script_ids, "auth_key":auth_key,
        "listen_host":"127.0.0.1", "http_port":18085, "socks5_port":18080,
        "verify_ssl":true, "lan_sharing":false, "relay_ip_literals":true,
        "relay_timeout":55, "tls_connect_timeout":20, "tcp_connect_timeout":15,
        "parallel_relay":1, "h2_connections":1, "enable_batch":true, "log_level":"INFO"
    }))
}
async fn start_mhr(options: &Value, app: AppHandle, core: &Core) -> Result<(), String> {
    let source = core.assets.join("mhr");
    let main = source.join("main.py");
    if !main.is_file() {
        return Err("MHR Python runtime files are missing from this build".into());
    }
    let config = mhr_config(options)?;
    let work = data_dir().join("mhr");
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    fs::write(
        work.join("config.json"),
        serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut processes = core.mhr_child.lock().await;
    if let Some(mut existing) = processes.take() {
        let _ = existing.kill().await;
    }
    let main_path = main.to_string_lossy().into_owned();
    let config_path = work.join("config.json");
    let mut child = Command::new("python3")
        .current_dir(&source)
        .args([
            main_path.as_str(),
            "--config",
            config_path.to_string_lossy().as_ref(),
            "--no-cert-check",
        ])
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONPATH", source.join("vendor"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("MHR needs Python 3.10 or newer: {e}"))?;
    if let Some(out) = child.stdout.take() {
        stream_logs(BufReader::new(out), app.clone(), "MHR", "info");
    }
    if let Some(err) = child.stderr.take() {
        stream_logs(BufReader::new(err), app, "MHR", "error");
    }
    *processes = Some(child);
    drop(processes);
    for _ in 0..200 {
        if tokio::net::TcpStream::connect("127.0.0.1:18080")
            .await
            .is_ok()
        {
            return Ok(());
        }
        let mut processes = core.mhr_child.lock().await;
        if processes
            .as_mut()
            .is_none_or(|child| child.try_wait().ok().flatten().is_some())
        {
            *processes = None;
            return Err(
                "MHR stopped before opening its local SOCKS listener; see Logs for its error"
                    .into(),
            );
        }
        drop(processes);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let mut processes = core.mhr_child.lock().await;
    if let Some(mut child) = processes.take() {
        let _ = child.kill().await;
    }
    Err("MHR did not open its local SOCKS listener within 20 seconds".into())
}
async fn start_sni_spoof(
    fake_sni: &str,
    target_ip: &str,
    target_port: u16,
    app: AppHandle,
    core: &Core,
) -> Result<(), String> {
    let script = core.assets.join("sni-spoofing/main.py");
    if !script.is_file() {
        return Err("Linux SNI spoofing helper is missing from this build".into());
    }
    let work = data_dir().join("sni-spoofing");
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let config = json!({"LISTEN_HOST":"127.0.0.1","LISTEN_PORT":18090,"CONNECT_IP":target_ip,"CONNECT_PORT":target_port,"FAKE_SNI":fake_sni});
    fs::write(
        work.join("config.json"),
        serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut current = core.sni_child.lock().await;
    if let Some(mut child) = current.take() {
        let _ = child.kill().await;
    }
    let config_path = work.join("config.json");
    let mut child = Command::new("pkexec")
        .arg("python3")
        .arg(&script)
        .arg(&config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("SNI spoofing needs pkexec, Python 3, and CAP_NET_RAW: {e}"))?;
    if let Some(out) = child.stdout.take() {
        stream_logs(BufReader::new(out), app.clone(), "SNI Spoof", "info");
    }
    if let Some(err) = child.stderr.take() {
        stream_logs(BufReader::new(err), app, "SNI Spoof", "error");
    }
    *current = Some(child);
    drop(current);
    for _ in 0..100 {
        if tokio::net::TcpStream::connect("127.0.0.1:18090")
            .await
            .is_ok()
        {
            return Ok(());
        }
        let mut current = core.sni_child.lock().await;
        if current
            .as_mut()
            .is_none_or(|child| child.try_wait().ok().flatten().is_some())
        {
            *current = None;
            return Err(
                "SNI spoofing helper exited before opening its local relay; see Logs".into(),
            );
        }
        drop(current);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let mut current = core.sni_child.lock().await;
    if let Some(mut child) = current.take() {
        let _ = child.kill().await;
    }
    Err("SNI spoofing relay did not start within 10 seconds".into())
}
#[tauri::command]
async fn aether_register(
    kind: String,
    app: AppHandle,
    core: State<'_, Core>,
) -> Result<(), String> {
    if !matches!(
        kind.as_str(),
        "all" | "wg" | "masque" | "gool" | "gool-classic" | "mim"
    ) {
        return Err("Unknown Aether identity type".into());
    }
    if core
        .child
        .lock()
        .await
        .as_mut()
        .is_some_and(|p| p.try_wait().ok().flatten().is_none())
        || core
            .aether_child
            .lock()
            .await
            .as_mut()
            .is_some_and(|p| p.try_wait().ok().flatten().is_none())
    {
        return Err("Stop the active connection before registering Aether identities".into());
    }
    let work_dir = data_dir().join("aether");
    fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;
    let mut command = Command::new(&core.aether_binary);
    command
        .current_dir(&work_dir)
        .args(["--register", &kind])
        .env("AETHER_TOR_PT_DIR", core.assets.join("pt"));
    if core.assets.join("pt/psiphon-tunnel-core").is_file() {
        command.env(
            "AETHER_PSIPHON_BIN",
            core.assets.join("pt/psiphon-tunnel-core"),
        );
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not start Aether identity registration: {e}"))?;
    if let Some(out) = child.stdout.take() {
        stream_logs(BufReader::new(out), app.clone(), "Aether Identity", "info");
    }
    if let Some(err) = child.stderr.take() {
        stream_logs(BufReader::new(err), app, "Aether Identity", "error");
    }
    let status = child.wait().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Aether identity registration failed ({status})"))
    }
}
#[tauri::command]
fn aether_identities() -> Vec<Value> {
    let root = data_dir().join("aether");
    ["aether.toml", "aether-masque.toml", "aether-team.toml", "aether-gool.toml", "aether-mim.toml"]
        .into_iter().filter_map(|name| {
            let raw = fs::read_to_string(root.join(name)).ok()?;
            let field = |key: &str| raw.lines().find_map(|line| line.trim().strip_prefix(&format!("{key} = \""))?.strip_suffix('"').map(str::to_owned));
            Some(json!({"file":name,"device_id":field("device_id"),"ipv4":field("ipv4"),"ipv6":field("ipv6")}))
        }).collect()
}
#[tauri::command]
async fn start_core(
    id: String,
    mode: String,
    dns: String,
    dns_enabled: bool,
    route: String,
    share_lan: bool,
    aether_options: Value,
    mhr_options: Value,
    sni_options: Value,
    app: AppHandle,
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
        .ok_or("Config not found")?
        .clone();
    let sni_enabled = sni_options.get("enabled").and_then(Value::as_bool) == Some(true);
    if sni_enabled
        && matches!(
            p.kind.to_ascii_lowercase().as_str(),
            "sing-box" | "mihomo" | "aether" | "mhr" | "hysteria2" | "hy2" | "tuic"
        )
    {
        return Err("Linux SNI spoofing currently works with Xray TLS profiles (VLESS, VMess, or Trojan) only".into());
    }
    fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    let mut sni_target: Option<(String, u16)> = None;
    let (bin, path, engine, tun_elevated) = match p.kind.to_ascii_lowercase().as_str() {
        "sing-box" => {
            let config = p
                .uri
                .strip_prefix("sing-box://")
                .ok_or("Sing-box profile must contain a full sing-box JSON config")?;
            let value: Value =
                serde_json::from_str(config).map_err(|e| format!("Invalid sing-box JSON: {e}"))?;
            if !value
                .get("outbounds")
                .and_then(Value::as_array)
                .is_some_and(|outbounds| !outbounds.is_empty())
            {
                return Err("Sing-box config needs at least one outbound".into());
            }
            let path = data_dir().join("active-sing-box.json");
            fs::write(
                &path,
                serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            (&core.singbox_binary, path, "sing-box", mode == "tun")
        }
        "mihomo" => {
            let config = p
                .uri
                .strip_prefix("mihomo://")
                .ok_or("Mihomo profile must contain a full YAML config")?;
            if !config.lines().any(|line| {
                line.trim_start().starts_with("proxies:")
                    || line.trim_start().starts_with("proxy-providers:")
            }) {
                return Err("Mihomo YAML must define proxies or proxy-providers".into());
            }
            let path = data_dir().join("active-mihomo.yaml");
            fs::write(&path, config).map_err(|e| e.to_string())?;
            (&core.mihomo_binary, path, "mihomo", mode == "tun")
        }
        "hysteria2" | "hy2" | "tuic" => {
            let config = singbox_link_config(&p, &mode, share_lan, &route)?;
            let path = data_dir().join("active-sing-box.json");
            fs::write(
                &path,
                serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            (&core.singbox_binary, path, "sing-box", mode == "tun")
        }
        _ => {
            let mut conf = build_client_config(&p, &mode, &dns, dns_enabled, &route, share_lan)?;
            if sni_enabled {
                let first = conf["outbounds"]
                    .as_array_mut()
                    .and_then(|items| items.first_mut())
                    .ok_or("Xray outbound is missing")?;
                if first["streamSettings"]["security"].as_str() != Some("tls") {
                    return Err("SNI spoofing requires an Xray TLS profile; Reality and non-TLS profiles are not supported".into());
                }
                let host = first["settings"]["address"]
                    .as_str()
                    .ok_or("Could not read the Xray target host")?
                    .to_owned();
                let profile_port = first["settings"]["port"]
                    .as_u64()
                    .and_then(|value| u16::try_from(value).ok())
                    .ok_or("Could not read the Xray target port")?;
                let port = sni_options
                    .get("connect_port")
                    .and_then(Value::as_str)
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| {
                        let port = v
                            .trim()
                            .parse::<u16>()
                            .map_err(|_| "SNI target port must be between 1 and 65535")?;
                        if port == 0 {
                            return Err("SNI target port must be between 1 and 65535");
                        }
                        Ok(port)
                    })
                    .transpose()?
                    .unwrap_or(profile_port);
                let override_ip = sni_options
                    .get("connect_ip")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                let target_ip = if !override_ip.is_empty() {
                    override_ip
                        .parse::<std::net::Ipv4Addr>()
                        .map_err(|_| "SNI target override must be an IPv4 address")?
                        .to_string()
                } else if host.parse::<std::net::Ipv4Addr>().is_ok() {
                    host
                } else {
                    tokio::net::lookup_host((host.as_str(), port))
                        .await
                        .map_err(|e| format!("Could not resolve SNI target: {e}"))?
                        .find(|address| address.is_ipv4())
                        .ok_or("SNI spoofing currently requires an IPv4 target")?
                        .ip()
                        .to_string()
                };
                let fake_sni = sni_options
                    .get("fake_sni")
                    .and_then(Value::as_str)
                    .unwrap_or("mci.ir")
                    .trim();
                if fake_sni.is_empty()
                    || fake_sni.len() > 253
                    || !fake_sni
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte))
                {
                    return Err("Fake SNI must be a valid DNS hostname".into());
                }
                sni_target = Some((target_ip, port));
                first["settings"]["address"] = json!("127.0.0.1");
                first["settings"]["port"] = json!(18090);
            }
            let path = data_dir().join("active-xray.json");
            fs::write(
                &path,
                serde_json::to_vec_pretty(&conf).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            (&core.binary, path, "Xray", mode == "tun")
        }
    };
    let test = match engine {
        "sing-box" => {
            Command::new(bin)
                .args(["check", "-c"])
                .arg(&path)
                .output()
                .await
        }
        "mihomo" => {
            Command::new(bin)
                .args(["-t", "-f"])
                .arg(&path)
                .output()
                .await
        }
        _ => {
            Command::new(bin)
                .env("XRAY_LOCATION_ASSET", &core.assets)
                .args(["run", "-test", "-c"])
                .arg(&path)
                .output()
                .await
        }
    }
    .map_err(|e| format!("{engine} unavailable: {e}"))?;
    if !test.status.success() {
        return Err(String::from_utf8_lossy(&test.stderr).to_string());
    }
    if let Some((target_ip, target_port)) = sni_target {
        start_sni_spoof(
            sni_options
                .get("fake_sni")
                .and_then(Value::as_str)
                .unwrap_or("mci.ir"),
            &target_ip,
            target_port,
            app.clone(),
            &core,
        )
        .await?;
    } else if let Some(mut sni) = core.sni_child.lock().await.take() {
        let _ = sni.kill().await;
    }
    if p.kind.eq_ignore_ascii_case("mhr") {
        start_mhr(&mhr_options, app.clone(), &core).await?;
    } else if let Some(mut mhr) = core.mhr_child.lock().await.take() {
        let _ = mhr.kill().await;
    }
    let mut aether = core.aether_child.lock().await;
    if let Some(mut old) = aether.take() {
        let _ = old.kill().await;
    }
    if p.kind.eq_ignore_ascii_case("aether") {
        if !core.aether_binary.is_file() {
            return Err("Aether core is missing; rebuild or install the Aether binary".into());
        }
        if tokio::net::TcpStream::connect("127.0.0.1:1819")
            .await
            .is_ok()
        {
            return Err("Aether SOCKS port 1819 is already in use".into());
        }
        let args = aether_args(&p, &aether_options)?;
        let work_dir = data_dir().join("aether");
        fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;
        let mut aether_command = Command::new(&core.aether_binary);
        aether_command
            .current_dir(&work_dir)
            .env("AETHER_TOR_PT_DIR", core.assets.join("pt"));
        if core.assets.join("pt/psiphon-tunnel-core").is_file() {
            aether_command.env(
                "AETHER_PSIPHON_BIN",
                core.assets.join("pt/psiphon-tunnel-core"),
            );
        }
        let mut process = aether_command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start Aether core: {e}"))?;
        if let Some(stdout) = process.stdout.take() {
            stream_logs(BufReader::new(stdout), app.clone(), "Aether", "info");
        }
        if let Some(stderr) = process.stderr.take() {
            stream_logs(BufReader::new(stderr), app.clone(), "Aether", "error");
        }
        *aether = Some(process);
        drop(aether);
        let mut ready = false;
        for _ in 0..100 {
            if tokio::net::TcpStream::connect("127.0.0.1:1819")
                .await
                .is_ok()
            {
                ready = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        if !ready {
            let mut process = core.aether_child.lock().await;
            if let Some(mut child) = process.take() {
                let _ = child.kill().await;
            }
            return Err("Aether did not open its local SOCKS port".into());
        }
    } else {
        drop(aether);
    }
    let (run_args, run_config) = match engine {
        "sing-box" => (vec!["run".to_owned(), "-c".to_owned()], path.clone()),
        "mihomo" => (
            vec![
                "-d".to_owned(),
                data_dir().to_string_lossy().into_owned(),
                "-f".to_owned(),
            ],
            path.clone(),
        ),
        _ => (vec!["run".to_owned(), "-c".to_owned()], path.clone()),
    };
    let mut command = if tun_elevated {
        let mut elevated = Command::new("pkexec");
        if engine == "Xray" {
            elevated
                .arg("env")
                .arg(format!("XRAY_LOCATION_ASSET={}", core.assets.display()))
                .arg(bin);
        } else {
            elevated.arg(bin);
        }
        elevated
    } else {
        let mut ordinary = Command::new(bin);
        if engine == "Xray" {
            ordinary.env("XRAY_LOCATION_ASSET", &core.assets);
        }
        ordinary
    };
    let child = command
        .args(run_args)
        .arg(run_config)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            if p.kind.eq_ignore_ascii_case("aether") {
                let mut process = core.aether_child.lock().await;
                if let Some(mut child) = process.take() {
                    let _ = child.kill().await;
                }
            }
            if p.kind.eq_ignore_ascii_case("mhr") {
                if let Some(mut mhr) = core.mhr_child.lock().await.take() {
                    let _ = mhr.kill().await;
                }
            }
            if sni_enabled {
                if let Some(mut sni) = core.sni_child.lock().await.take() {
                    let _ = sni.kill().await;
                }
            }
            return Err(if tun_elevated {
                format!("TUN needs pkexec and a PolicyKit authentication agent: {error}")
            } else {
                error.to_string()
            });
        }
    };
    if let Some(stdout) = child.stdout.take() {
        stream_logs(BufReader::new(stdout), app.clone(), engine, "info");
    }
    if let Some(stderr) = child.stderr.take() {
        stream_logs(BufReader::new(stderr), app, engine, "error");
    }
    *c = Some(child);
    Ok(())
}
#[tauri::command]
async fn stop_core(core: State<'_, Core>) -> Result<(), String> {
    let mut c = core.child.lock().await;
    if let Some(mut p) = c.take() {
        p.kill().await.map_err(|e| e.to_string())?;
    }
    let mut aether = core.aether_child.lock().await;
    if let Some(mut p) = aether.take() {
        let _ = p.kill().await;
    }
    let mut mhr = core.mhr_child.lock().await;
    if let Some(mut p) = mhr.take() {
        let _ = p.kill().await;
    }
    let mut sni = core.sni_child.lock().await;
    if let Some(mut p) = sni.take() {
        let _ = p.kill().await;
    }
    Ok(())
}
#[tauri::command]
async fn core_running(core: State<'_, Core>) -> Result<bool, String> {
    let mut child = core.child.lock().await;
    let running = child.as_mut().is_some_and(|process| {
        process
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    });
    let mut aether = core.aether_child.lock().await;
    let aether_running = aether.as_mut().is_some_and(|process| {
        process
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    });
    let mut mhr = core.mhr_child.lock().await;
    let mhr_running = mhr.as_mut().is_some_and(|process| {
        process
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    });
    if !running {
        *child = None;
    }
    if !aether_running {
        *aether = None;
    }
    if !mhr_running {
        *mhr = None;
    }
    let mut sni = core.sni_child.lock().await;
    let sni_running = sni.as_mut().is_some_and(|process| {
        process
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    });
    if !sni_running {
        *sni = None;
    }
    Ok(running || aether_running || mhr_running || sni_running)
}
#[tauri::command]
async fn core_version(core: State<'_, Core>) -> Result<String, String> {
    let output = Command::new(&core.binary)
        .arg("version")
        .output()
        .await
        .map_err(|e| format!("Could not run Xray: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("—")
        .to_string())
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
async fn ping_all(
    provider: String,
    core: State<'_, Core>,
) -> Result<Vec<(String, Option<u32>)>, String> {
    let s = read_store();
    let mut out = Vec::new();
    for p in s.profiles {
        let ms = ping_profile(&core, &p, &provider).await;
        out.push((p.id, ms));
    }
    Ok(out)
}

// Run an isolated Xray instance for each profile and measure a real HTTPS
// response through its local SOCKS inbound, following PattN's test-core model.
async fn ping_profile(core: &Core, profile: &Profile, provider: &str) -> Option<u32> {
    let outbound = outbound(profile).ok()?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.ok()?;
    let port = listener.local_addr().ok()?.port();
    drop(listener);
    let config = json!({
        "log":{"loglevel":"none"},
        "inbounds":[{"listen":"127.0.0.1","port":port,"protocol":"socks","settings":{"udp":false}}],
        "outbounds":[outbound]
    });
    fs::create_dir_all(data_dir()).ok()?;
    let path = data_dir().join(format!("ping-{}.json", uuid::Uuid::new_v4()));
    fs::write(&path, serde_json::to_vec(&config).ok()?).ok()?;
    let test = Command::new(&core.binary)
        .env("XRAY_LOCATION_ASSET", &core.assets)
        .args(["run", "-test", "-c"])
        .arg(&path)
        .output()
        .await
        .ok()?;
    if !test.status.success() {
        let _ = fs::remove_file(path);
        return None;
    }
    let child = Command::new(&core.binary)
        .env("XRAY_LOCATION_ASSET", &core.assets)
        .args(["run", "-c"])
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(_) => {
            let _ = fs::remove_file(path);
            return None;
        }
    };
    let proxy = match reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}")) {
        Ok(proxy) => proxy,
        Err(_) => {
            let _ = child.kill().await;
            let _ = fs::remove_file(path);
            return None;
        }
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .proxy(proxy)
        .build();
    let client = match client {
        Ok(client) => client,
        Err(_) => {
            let _ = child.kill().await;
            let _ = fs::remove_file(path);
            return None;
        }
    };
    let mut result = None;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    while std::time::Instant::now() < deadline {
        let started = std::time::Instant::now();
        match client.get(provider).send().await {
            Ok(response) if response.status().is_success() => {
                result = Some(started.elapsed().as_millis().min(u32::MAX as u128) as u32);
                break;
            }
            _ => tokio::time::sleep(std::time::Duration::from_millis(150)).await,
        }
    }
    let _ = child.kill().await;
    let _ = fs::remove_file(path);
    result
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
            let local_aether = std::env::current_dir()
                .unwrap_or_default()
                .join("src-tauri/resources/aether");
            let local_singbox = std::env::current_dir()
                .unwrap_or_default()
                .join("src-tauri/resources/sing-box");
            let local_mihomo = std::env::current_dir()
                .unwrap_or_default()
                .join("src-tauri/resources/mihomo");
            let binary = [resource.join("xray"), local_binary, PathBuf::from("xray")]
                .into_iter()
                .find(|path| path.is_file())
                .unwrap_or_else(|| PathBuf::from("xray"));
            let aether_binary = [
                resource.join("aether"),
                local_aether,
                PathBuf::from("aether"),
            ]
            .into_iter()
            .find(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("aether"));
            let singbox_binary = [
                resource.join("sing-box"),
                local_singbox,
                PathBuf::from("sing-box"),
            ]
            .into_iter()
            .find(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("sing-box"));
            let mihomo_binary = [
                resource.join("mihomo"),
                local_mihomo,
                PathBuf::from("mihomo"),
            ]
            .into_iter()
            .find(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("mihomo"));
            app.manage(Core {
                child: AsyncMutex::new(None),
                aether_child: AsyncMutex::new(None),
                mhr_child: AsyncMutex::new(None),
                sni_child: AsyncMutex::new(None),
                binary,
                aether_binary,
                singbox_binary,
                mihomo_binary,
                assets: resource,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            client_secrets,
            save_client_secrets,
            list_data,
            import_text,
            save_profile,
            update_profile,
            delete_profile,
            clear_all,
            clear_stats,
            export_backup,
            import_backup,
            open_storage_location,
            add_subscription,
            refresh_subscriptions,
            aether_scan,
            aether_register,
            aether_identities,
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
