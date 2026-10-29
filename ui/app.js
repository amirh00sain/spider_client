const invoke = (...args) => window.__TAURI__.core.invoke(...args);
const $ = s => document.querySelector(s);
const $$ = s => [...document.querySelectorAll(s)];
const text = {
  fa: { ready:'آماده', connecting:'در حال اتصال', connected:'متصل', disconnected:'قطع', none:'بدون کانفیگ', choose:'انتخاب کانفیگ', select:'برای شروع کانفیگ انتخاب کنید', configs:'کانفیگ‌ها', settings:'تنظیمات', logs:'لاگ‌ها', home:'خانه', search:'جستجو', empty:'کانفیگی نیست', add:'افزودن', paste:'کلیپ‌بورد', sub:'ساب', manual:'دستی', library:'کتابخانه', preferences:'تنظیمات', core:'هسته', version:'نسخه Xray', update:'بررسی', dns:'DNS', route:'مسیر', iran:'دامنه‌های ایران مستقیم', speed:'تست سرعت', clear:'حذف همه', clearNote:'کانفیگ‌ها و ساب‌ها', subscription:'Subscription', uri:'URI · JSON · Base64', name:'نام', link:'پیوند', protocol:'پروتکل', save:'ذخیره', clipboard:'از کلیپ‌بورد', addNow:'افزودن', connect:'اتصال', copy:'کپی', edit:'ویرایش', remove:'حذف', pingDone:'پینگ انجام شد', saved:'ذخیره شد', deleted:'حذف شد', copied:'کپی شد', updateFound:'آخرین نسخه Xray', noConfig:'ابتدا کانفیگ انتخاب کنید', tunPermission:'TUN به مجوز شبکه نیاز دارد', cleared:'پاک شد', confirmClear:'همه کانفیگ‌ها و ساب‌ها حذف شوند؟', clipError:'کلیپ‌بورد در دسترس نیست', direct:'مستقیم', global:'سراسری' },
  en: { ready:'READY', connecting:'CONNECTING', connected:'CONNECTED', disconnected:'DISCONNECTED', none:'No config', choose:'Select config', select:'Choose a config to start', configs:'Configs', settings:'Settings', logs:'Logs', home:'Home', search:'Search', empty:'No configs', add:'Add', paste:'Clipboard', sub:'Sub', manual:'Manual', library:'LIBRARY', preferences:'PREFERENCES', core:'Core', version:'Xray version', update:'Check', dns:'DNS', route:'Route', iran:'Iran domains direct', speed:'Speed test', clear:'Clear all', clearNote:'Configs and subscriptions', subscription:'Subscription', uri:'URI · JSON · Base64', name:'Name', link:'Link', protocol:'Protocol', save:'Save', clipboard:'Paste clipboard', addNow:'Add', connect:'Connect', copy:'Copy', edit:'Edit', remove:'Delete', pingDone:'Ping complete', saved:'Saved', deleted:'Deleted', copied:'Copied', updateFound:'Latest Xray', noConfig:'Select a config first', tunPermission:'TUN needs network permission', cleared:'Cleared', confirmClear:'Delete all configs and subscriptions?', clipError:'Clipboard unavailable', direct:'Direct', global:'Global' }
};
let lang = localStorage.lang || 'fa';
let theme = localStorage.theme || 'dark';
let mode = 'proxy';
let dnsEnabled = localStorage.dnsEnabled !== 'false';
let shareLan = localStorage.shareLan === 'true';
let state = 'disconnected';
let data = { profiles: [], subs: [] };
let selected = null;
let editingId = null;
let manualDirty = false;
let started = 0;
let toastTimer;
const logEntries = [];
let liveLogs = localStorage.liveLogs !== 'false';
const tr = key => text[lang][key] || key;

function setupSettingsAccordions() {
  $$('#settings .settings-group').forEach((group, index) => {
    if (group.tagName === 'DETAILS') return;
    const details = document.createElement('details');
    details.className = `${group.className} settings-accordion`;
    details.open = index === 0 || group.classList.contains('aether-settings');
    const summary = document.createElement('summary');
    summary.className = 'group-head';
    const heading = group.querySelector('.group-head');
    if (heading) summary.append(...heading.childNodes);
    details.append(summary);
    while (group.firstChild) {
      if (group.firstChild === heading) { group.removeChild(heading); continue; }
      details.append(group.firstChild);
    }
    group.replaceWith(details);
  });
}
setupSettingsAccordions();
function addEngineSettings() {
  const scroll = $('#settings .settings-scroll');
  const anchor = [...scroll.querySelectorAll('summary .group-head, summary')].find(node => node.textContent.includes('DIAGNOSTICS'))?.closest('details');
  const html = `<details class="settings-group glass settings-accordion mhr-settings"><summary class="group-head"><span>MHR RELAY</span><small>APPS SCRIPT · DOMAIN FRONT</small></summary><div class="manual-fields"><label><span>DEPLOYMENT ID</span><input class="inline-input" id="mhr-script-id" placeholder="Google Apps Script deployment ID"></label><label><span>AUTH KEY</span><input class="inline-input" id="mhr-auth-key" type="password" placeholder="Must match Code.gs AUTH_KEY"></label></div><small class="aether-note">Deploy the Apps Script as a Web App (execute as you, access Anyone), then enter its Deployment ID and matching AUTH_KEY. MHR starts when its profile connects; the local SOCKS port stays on loopback.</small><div class="engine-actions"><button class="small-action" id="mhr-save">Save relay credentials</button><button class="small-action" data-engine-profile="mhr">Create MHR profile</button></div></details><details class="settings-group glass settings-accordion engine-settings"><summary class="group-head"><span>ENGINE PROFILES</span><small>SING-BOX · MIHOMO</small></summary><div class="engine-card"><b>sing-box</b><small>Full JSON profile for Hysteria2, TUIC, WireGuard and sing-box routing.</small><button class="small-action" data-engine-profile="sing-box">Add sing-box config</button></div><div class="engine-card"><b>mihomo</b><small>Full Clash Meta YAML profile for proxies, providers, rules and TUN.</small><button class="small-action" data-engine-profile="mihomo">Add mihomo config</button></div></details>`;
  if (anchor) anchor.insertAdjacentHTML('beforebegin', html); else scroll.insertAdjacentHTML('beforeend', html);
  const aether = scroll.querySelector('.aether-settings');
  aether.insertAdjacentHTML('afterend', `<details class="settings-group glass settings-accordion" id="tor-settings"><summary class="group-head"><span>TOR</span><small>BUILT-IN AETHER CARRIER</small></summary><div class="manual-fields" id="tor-fields"></div><small class="aether-note">Tor is provided by the Aether core. Tor-only and reverse modes require the bundled Tor support files in the release package.</small></details><details class="settings-group glass settings-accordion" id="psiphon-settings"><summary class="group-head"><span>PSIPHON SHIROKHORSHID</span><small>REGION · CDN FRONTING</small></summary><div class="manual-fields" id="psiphon-fields"></div><small class="aether-note">Psiphon runs inside Aether. Region codes use ISO country codes; CDN settings are optional overrides.</small></details><details class="settings-group glass settings-accordion" id="zerotrust-settings"><summary class="group-head"><span>CLOUDFLARE ZERO TRUST</span><small>TEAM IDENTITY</small></summary><div class="manual-fields" id="zerotrust-fields"></div><button class="small-action" id="save-zero-trust">Save identity settings</button><small class="aether-note">Use one sign-in method: one-time email, service token ID/secret, or an access JWT. Credentials are stored in a private local app settings file.</small></details><details class="settings-group glass settings-accordion" id="sni-settings"><summary class="group-head"><span>SNI SPOOFING</span><small>LINUX · WRONG-SEQUENCE TLS</small></summary><div class="setting-card compact-setting"><div class="setting-title"><span class="setting-icon violet"><svg viewBox="0 0 24 24"><path d="M12 3 4 7v5c0 5 3.4 8 8 9 4.6-1 8-4 8-9V7l-8-4Z"/><path d="m9 12 2 2 4-4"/></svg></span><div><b>Enable SNI spoofing</b><small>TCP fake ClientHello · no server</small></div></div><button class="switch" id="sni-enabled" aria-pressed="false"><i></i></button></div><div class="manual-fields"><label><span>FAKE SNI</span><input class="inline-input" id="sni-fake-host" value="mci.ir" placeholder="example.com"></label><label><span>CONNECT IP OVERRIDE</span><input class="inline-input" id="sni-target-ip" placeholder="auto · active profile IP"></label><label><span>CONNECT PORT OVERRIDE</span><input class="inline-input" id="sni-target-port" inputmode="numeric" placeholder="auto · profile port"></label></div><small class="aether-note">Linux implementation of MehrON’s wrong-sequence fake ClientHello. Uses a local relay and asks for administrator permission (pkexec/CAP_NET_RAW). Supports IPv4 Xray TLS VLESS, VMess and Trojan profiles; Reality, Aether and custom sing-box/mihomo profiles are not supported. Use the normal server SNI on the profile.</small></details>`);
  for (const [id, container] of [['aether-tor','tor-fields'], ['aether-psiphon','psiphon-fields'], ['aether-psiphon-mode','psiphon-fields'], ['aether-psiphon-region','psiphon-fields'], ['aether-cdn-ips','psiphon-fields'], ['aether-cdn-sni','psiphon-fields'], ['aether-team','zerotrust-fields'], ['aether-access-email','zerotrust-fields'], ['aether-access-id','zerotrust-fields'], ['aether-access-secret','zerotrust-fields'], ['aether-access-token','zerotrust-fields']]) {
    const field = $(`#${id}`)?.closest('label');
    if (field) $(`#${container}`).append(field);
  }
  $('#mhr-save').onclick = async () => {
    try {
      await invoke('save_client_secrets', { values: { mhr_script_id: $('#mhr-script-id').value.trim(), mhr_auth_key: $('#mhr-auth-key').value } });
      say(lang === 'fa' ? 'اعتبارهای MHR در فضای امن برنامه ذخیره شد' : 'MHR credentials saved privately');
    } catch (error) { say(String(error)); }
  };
  $('#save-zero-trust').onclick = async () => {
    try {
      await invoke('save_client_secrets', { values: {
        aether_access_email: $('#aether-access-email').value.trim(), aether_access_id: $('#aether-access-id').value.trim(),
        aether_access_secret: $('#aether-access-secret').value, aether_access_token: $('#aether-access-token').value
      } });
      say(lang === 'fa' ? 'شناسه Zero Trust ذخیره شد' : 'Zero Trust identity saved');
    } catch (error) { say(String(error)); }
  };
  $('#sni-fake-host').value = localStorage.sniFakeHost || 'mci.ir';
  $('#sni-target-ip').value = localStorage.sniTargetIp || '';
  $('#sni-target-port').value = localStorage.sniTargetPort || '';
  $('#sni-enabled').classList.toggle('on', localStorage.sniSpoofEnabled === 'true');
  $('#sni-enabled').setAttribute('aria-pressed', String(localStorage.sniSpoofEnabled === 'true'));
  $('#sni-enabled').onclick = () => {
    const enabled = !$('#sni-enabled').classList.contains('on');
    $('#sni-enabled').classList.toggle('on', enabled);
    $('#sni-enabled').setAttribute('aria-pressed', String(enabled));
    localStorage.sniSpoofEnabled = String(enabled);
    localStorage.sniFakeHost = $('#sni-fake-host').value.trim() || 'mci.ir';
    if (state === 'connected') { stopCore().then(startSelected); }
  };
  $('#sni-fake-host').onchange = () => {
    localStorage.sniFakeHost = $('#sni-fake-host').value.trim() || 'mci.ir';
    if (state === 'connected' && $('#sni-enabled').classList.contains('on')) { stopCore().then(startSelected); }
  };
  for (const id of ['sni-target-ip','sni-target-port']) $(`#${id}`).onchange = () => {
    localStorage[id === 'sni-target-ip' ? 'sniTargetIp' : 'sniTargetPort'] = $(`#${id}`).value.trim();
    if (state === 'connected' && $('#sni-enabled').classList.contains('on')) { stopCore().then(startSelected); }
  };
  $$('[data-engine-profile]').forEach(button => button.onclick = () => {
    openModal('manual');
    $('#manual-kind').value = button.dataset.engineProfile;
    $('#manual-name').value = `${button.dataset.engineProfile} profile`;
    $('#manual-uri').value = button.dataset.engineProfile === 'mhr' ? 'mhr://local' : '';
    manualDirty = true;
    updateManualFields();
  });
}
addEngineSettings();

function renderLogs() {
  $('#logs-live').classList.toggle('on', liveLogs);
  $('#logs-live').setAttribute('aria-pressed', String(liveLogs));
  $('#logs-live-label').textContent = lang === 'fa' ? (liveLogs ? 'زنده' : 'متوقف') : (liveLogs ? 'Live' : 'Paused');
  const list = $('#log-list');
  list.innerHTML = logEntries.length ? logEntries.map(entry => `<article class="log-entry ${esc(entry.level)}"><time>${esc(new Date(Number(entry.time) * 1000).toLocaleTimeString())}</time><b>${esc(entry.source)}</b><span>${esc(entry.message)}</span></article>`).join('') : `<div class="logs-empty">${lang === 'fa' ? 'هنوز لاگی ثبت نشده است' : 'No logs yet'}</div>`;
  if (liveLogs) list.scrollTop = list.scrollHeight;
}
async function refreshAetherIdentities() {
  try {
    const identities = await invoke('aether_identities');
    $('#aether-identities').innerHTML = identities.length ? identities.map(item => `<div><b>${esc(item.file)}</b><span>${esc(item.device_id || '—')} · ${esc(item.ipv4 || '—')} · ${esc(item.ipv6 || '—')}</span></div>`).join('') : (lang === 'fa' ? 'هنوز identity ثبت نشده؛ نوع را انتخاب و Register را بزنید.' : 'No identity registered. Choose a type and register.');
  } catch (error) { $('#aether-identities').textContent = String(error); }
}
window.__TAURI__.event.listen('core-log', event => {
  const item = event.payload || {};
  logEntries.push({ time: item.time || Math.floor(Date.now() / 1000), source: item.source || 'Core', level: item.level || 'info', message: item.message || '' });
  if (logEntries.length > 1500) logEntries.splice(0, logEntries.length - 1500);
  if (liveLogs) renderLogs();
});

function loadAetherSettings() {
  for (const [id, key, fallback] of [
    ['aether-protocol','aetherProtocol','masque'], ['aether-transport','aetherTransport','h3'],
    ['aether-scan','aetherScan','balanced'], ['aether-noize','aetherNoize','auto'],
    ['aether-ip','aetherIp','both'], ['aether-sni','aetherSni','www.cloudflare.com'], ['aether-dns','aetherDns',''],
    ['aether-exit-loc','aetherExitLoc',''], ['aether-tor','aetherTor','off'], ['aether-psiphon','aetherPsiphon','off'],
    ['aether-psiphon-mode','aetherPsiphonMode','auto'], ['aether-psiphon-region','aetherPsiphonRegion',''],
    ['aether-cdn-ips','aetherCdnIps',''], ['aether-cdn-sni','aetherCdnSni',''], ['aether-team','aetherTeam',''],
  ]) if (localStorage[key] != null) $(`#${id}`).value = localStorage[key];
  $('#aether-ech').checked = localStorage.aetherEch === 'true';
  $('#aether-fragment').checked = localStorage.aetherFragment === 'true';
  invoke('client_secrets').then(secrets => {
    $('#mhr-script-id').value = secrets.mhr_script_id || '';
    $('#mhr-auth-key').value = secrets.mhr_auth_key || '';
    $('#aether-access-email').value = secrets.aether_access_email || '';
    $('#aether-access-id').value = secrets.aether_access_id || '';
    $('#aether-access-secret').value = secrets.aether_access_secret || '';
    $('#aether-access-token').value = secrets.aether_access_token || '';
  }).catch(error => say(String(error)));
}
function saveAetherSettings() {
  for (const [id, key] of [
    ['aether-protocol','aetherProtocol'], ['aether-transport','aetherTransport'], ['aether-scan','aetherScan'],
    ['aether-noize','aetherNoize'], ['aether-ip','aetherIp'], ['aether-sni','aetherSni'], ['aether-dns','aetherDns']
    ,['aether-exit-loc','aetherExitLoc'], ['aether-tor','aetherTor'], ['aether-psiphon','aetherPsiphon'],
    ['aether-psiphon-mode','aetherPsiphonMode'], ['aether-psiphon-region','aetherPsiphonRegion'],
    ['aether-cdn-ips','aetherCdnIps'], ['aether-cdn-sni','aetherCdnSni'], ['aether-team','aetherTeam']
  ]) localStorage[key] = $(`#${id}`).value;
  localStorage.aetherEch = String($('#aether-ech').checked);
  localStorage.aetherFragment = String($('#aether-fragment').checked);
  invoke('save_client_secrets', { values: {
    aether_access_email: $('#aether-access-email').value.trim(), aether_access_id: $('#aether-access-id').value.trim(),
    aether_access_secret: $('#aether-access-secret').value, aether_access_token: $('#aether-access-token').value
  } }).catch(error => say(String(error)));
}
loadAetherSettings();

function say(message) {
  $('#toast').textContent = message;
  $('#toast').classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $('#toast').classList.remove('show'), 2600);
}
function esc(value) {
  return String(value || '').replace(/[&<>"']/g, ch => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[ch]));
}
function wave(x = innerWidth / 2, y = innerHeight / 2) {
  const ring = document.createElement('i');
  ring.className = 'ripple';
  ring.style.left = `${x}px`;
  ring.style.top = `${y}px`;
  $('#ripples').append(ring);
  setTimeout(() => ring.remove(), 1500);
}
function setState(next) {
  state = next;
  document.body.classList.remove('connecting', 'connected', 'disconnected');
  document.body.classList.add(next);
  const key = next === 'connecting' ? 'connecting' : next === 'connected' ? 'connected' : next === 'disconnected' ? 'disconnected' : 'ready';
  $('#status').textContent = tr(key);
  $('.globe-wrap').classList.toggle('connecting', next === 'connecting');
  if (next === 'connected') { started = Date.now(); wave(); }
  if (next === 'disconnected') wave();
}
function render() {
  const query = $('#search').value.toLowerCase();
  const profiles = data.profiles.filter(p => p.name.toLowerCase().includes(query) || p.kind.toLowerCase().includes(query));
  $('#profile-list').innerHTML = profiles.map((p, i) => `<article class="profile-row glass" style="animation-delay:${i * 35}ms" data-id="${esc(p.id)}"><div class="proto">${esc(p.kind.slice(0,2).toUpperCase())}</div><div class="row-text"><b>${esc(p.name)}</b><small>${esc(p.kind.toUpperCase())} · ${esc(p.source === 'local' ? 'LOCAL' : p.source === 'manual' ? 'MANUAL' : (data.subs.find(sub => sub.id === p.source)?.name || 'SUB'))}</small></div><span class="ping-val">${p.latency == null ? '' : `${p.latency} ms`}</span><div class="row-actions"><button class="use" title="${tr('connect')}" aria-label="${tr('connect')}" data-act="use"><svg viewBox="0 0 24 24"><path d="m8 5 11 7-11 7V5Z"/></svg></button><button title="${tr('edit')}" aria-label="${tr('edit')}" data-act="edit"><svg viewBox="0 0 24 24"><path d="m14 5 5 5M4 20l4.5-1 10-10a2.1 2.1 0 0 0-3-3l-10 10L4 20Z"/></svg></button><button title="${tr('copy')}" aria-label="${tr('copy')}" data-act="copy"><svg viewBox="0 0 24 24"><rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/></svg></button><button title="${tr('remove')}" aria-label="${tr('remove')}" data-act="delete"><svg viewBox="0 0 24 24"><path d="M4 7h16m-10 4v6m4-6v6M6 7l1 14h10l1-14M9 7V4h6v3"/></svg></button></div></article>`).join('');
  $('#empty').style.display = profiles.length ? 'none' : 'flex';
  $('#profile-list').style.display = profiles.length ? 'block' : 'none';
  const current = data.profiles.find(p => p.id === selected);
  if (current) {
    $('#profile-title').textContent = current.name;
    $('#profile-subtitle').textContent = `${current.kind.toUpperCase()} · ${current.latency == null ? '— ms' : `${current.latency} ms`}`;
    $('#active-name').textContent = current.name;
  } else {
    selected = null;
    $('#profile-title').textContent = tr('choose');
    $('#profile-subtitle').textContent = tr('select');
    $('#active-name').textContent = tr('none');
  }
}
async function load() {
  try { data = await invoke('list_data'); } catch { data = { profiles: [], subs: [] }; }
  render();
  try { $('#core-version').textContent = await invoke('core_version'); } catch { $('#core-version').textContent = 'Xray —'; }
}
function switchTab(tab) {
  $$('.modal-tabs button').forEach(button => button.classList.toggle('active', button.dataset.tab === tab));
  $$('.modal-body').forEach(body => body.classList.toggle('hidden', body.id !== `tab-${tab}`));
}
function openModal(tab = 'paste') {
  editingId = null;
  $('#modal-title').textContent = tr('add');
  $('#manual-name').value = '';
  $('#manual-uri').value = '';
  setBasicFields('', '', '', '');
  setDetailFields();
  $('#manual-aether-protocol').value = $('#aether-protocol').value;
  $('#manual-aether-transport').value = $('#aether-transport').value;
  $('#manual-aether-scan').value = $('#aether-scan').value;
  $('#manual-aether-noize').value = $('#aether-noize').value;
  $('#manual-aether-sni').value = $('#aether-sni').value;
  $('#manual-aether-dns').value = $('#aether-dns').value;
  manualDirty = false;
  updateManualFields();
  switchTab(tab);
  $('#modal').classList.add('open');
}
function openEditor(profile) {
  editingId = profile.id;
  $('#modal-title').textContent = tr('edit');
  $('#manual-name').value = profile.name;
  $('#manual-uri').value = profile.uri;
  $('#manual-kind').value = profile.kind;
  updateManualFields();
  fillBasicFields(profile);
  manualDirty = false;
  switchTab('manual');
  $('#modal').classList.add('open');
}
function updateManualFields() {
  const kind = $('#manual-kind').value;
  $('#tab-manual').classList.toggle('wireguard-edit', kind === 'wireguard');
  $('#tab-manual').classList.toggle('aether-edit', kind === 'aether');
  $('#tab-manual').classList.toggle('engine-config-edit', kind === 'sing-box' || kind === 'mihomo');
  $('#manual-uri').placeholder = kind === 'sing-box' ? '{ "log": {}, "inbounds": [], "outbounds": [{ "type": "hysteria2", … }] }' : kind === 'mihomo' ? 'mixed-port: 10808\nmode: rule\nproxies:\n  - name: …' : 'vless://…';
}
function setBasicFields(host, port, user, secret) {
  $('#manual-host').value = host || '';
  $('#manual-port').value = port || '';
  $('#manual-user').value = user || '';
  $('#manual-secret').value = secret || '';
}
function setDetailFields(values = {}) {
  $('#manual-transport').value = values.transport || 'tcp';
  $('#manual-header-type').value = values.headerType || 'none';
  $('#manual-security').value = values.security || '';
  $('#manual-sni').value = values.sni || '';
  $('#manual-host-header').value = values.host || '';
  $('#manual-path').value = values.path || '';
  $('#manual-flow').value = values.flow || '';
  $('#manual-fingerprint').value = values.fp || '';
  $('#manual-public-key').value = values.pbk || '';
  $('#manual-short-id').value = values.sid || '';
  $('#manual-spiderx').value = values.spx || '';
  $('#manual-alpn').value = Array.isArray(values.alpn) ? values.alpn.join(',') : (values.alpn || '');
  $('#manual-encryption').value = values.encryption || '';
  $('#manual-insecure').checked = values.insecure === true || values.insecure === '1' || values.insecure === 'true';
  $('#manual-wg-address').value = values.wgAddress || '';
  $('#manual-wg-allowed').value = values.wgAllowed || '';
  $('#manual-wg-psk').value = values.wgPsk || '';
  $('#manual-wg-dns').value = values.wgDns || '';
  $('#manual-wg-mtu').value = values.wgMtu || '';
  $('#manual-wg-reserved').value = values.wgReserved || '';
}
function fillBasicFields(profile) {
  setBasicFields('', '', '', '');
  setDetailFields();
  try {
    if (profile.kind === 'aether') {
      const u = new URL(profile.uri);
      setBasicFields(u.hostname, u.port, '', '');
      const q = u.searchParams;
      $('#manual-aether-protocol').value = q.get('protocol') || $('#aether-protocol').value;
      $('#manual-aether-transport').value = q.get('transport') || $('#aether-transport').value;
      $('#manual-aether-scan').value = q.get('scan') || $('#aether-scan').value;
      $('#manual-aether-noize').value = q.get('noize') || $('#aether-noize').value;
      $('#manual-aether-sni').value = q.get('sni') || $('#aether-sni').value;
      $('#manual-aether-dns').value = q.get('dns') || $('#aether-dns').value;
      return;
    }
    if (profile.uri.startsWith('vmess://')) {
      const raw = decodeURIComponent(escape(atob(profile.uri.slice(8).split('#')[0])));
      const v = JSON.parse(raw);
      setBasicFields(v.add, v.port, v.id, v.id);
      setDetailFields({ transport: v.net === 'raw' ? 'tcp' : v.net, headerType: v.type, security: v.tls, sni: v.sni, host: v.host, path: v.path, fp: v.fp, spx: v.spx, alpn: v.alpn, encryption: v.scy, insecure: v.insecure });
      return;
    }
    if (profile.uri.trimStart().startsWith('{')) {
      const v = JSON.parse(profile.uri), s = v.settings || {};
      const peer = s.peers?.[0];
      const endpoint = peer?.endpoint || '';
      const endpointAt = endpoint.startsWith('[') ? endpoint.lastIndexOf(']:') : endpoint.lastIndexOf(':');
      const host = endpoint ? endpoint.slice(0, endpointAt).replace(/^\[|\]$/g, '') : (typeof s.address === 'string' ? s.address : s.server || s.vnext?.[0]?.address);
      const port = (endpoint ? endpoint.slice(endpointAt + (endpoint.startsWith('[') ? 2 : 1)) : '') || s.port || s.vnext?.[0]?.port;
      const user = peer?.publicKey || s.id || s.vnext?.[0]?.users?.[0]?.id;
      const secret = s.password || s.secretKey || s.vnext?.[0]?.users?.[0]?.password;
      setBasicFields(host, port, user, secret);
      const stream = v.streamSettings || {}, tls = stream.tlsSettings || {}, reality = stream.realitySettings || {};
      setDetailFields({ transport: stream.method === 'raw' ? 'tcp' : stream.method, headerType: stream.rawSettings?.header?.type || stream.kcpSettings?.header?.type, security: stream.security, sni: tls.serverName || reality.serverName, host: stream.wsSettings?.host || stream.wsSettings?.headers?.Host || stream.xhttpSettings?.host || stream.httpupgradeSettings?.host, path: stream.wsSettings?.path || stream.rawSettings?.header?.request?.path?.[0] || stream.kcpSettings?.seed || stream.xhttpSettings?.path || stream.grpcSettings?.serviceName || stream.httpupgradeSettings?.path, flow: s.flow, fp: reality.fingerprint, pbk: reality.publicKey, sid: reality.shortId, spx: reality.spiderX, alpn: tls.alpn, encryption: s.encryption || s.method || s.security, wgAddress: Array.isArray(s.address) ? s.address.join(', ') : '', wgAllowed: peer?.allowedIPs?.join(', '), wgPsk: peer?.preSharedKey, wgDns: s.remoteDNS?.join(', '), wgMtu: s.mtu, wgReserved: s.reserved?.join(', ') });
      return;
    }
    if (profile.uri.startsWith('ss://')) {
      let raw = profile.uri.slice(5).split('#')[0].split('?')[0];
      let decoded = raw;
      if (!decoded.includes('@')) {
        raw = raw.replace(/-/g,'+').replace(/_/g,'/');
        raw += '='.repeat((4 - raw.length % 4) % 4);
        decoded = decodeURIComponent(escape(atob(raw)));
      }
      let [auth, endpoint] = decoded.split('@');
      if (endpoint && !auth.includes(':')) {
        let encoded = auth.replace(/-/g,'+').replace(/_/g,'/');
        encoded += '='.repeat((4 - encoded.length % 4) % 4);
        auth = decodeURIComponent(escape(atob(encoded)));
      }
      const [method, password] = auth.split(':');
      const pivot = endpoint.lastIndexOf(':');
      const host = endpoint.slice(0, pivot).replace(/^\[|\]$/g, '');
      const port = endpoint.slice(pivot + 1);
      setBasicFields(host, port, method, password);
      setDetailFields({ encryption: method });
      return;
    }
    const u = new URL(profile.uri);
    const host = u.hostname;
    const port = u.port;
    const user = decodeURIComponent(u.username || '');
    const secret = decodeURIComponent(u.password || '');
    setBasicFields(host, port, user, secret);
    const q = u.searchParams;
    setDetailFields({ transport: q.get('type') === 'raw' ? 'tcp' : q.get('type'), headerType: q.get('headerType'), security: q.get('security'), sni: q.get('sni'), host: q.get('host'), path: q.get('path') || q.get('serviceName') || q.get('seed'), flow: q.get('flow'), fp: q.get('fp'), pbk: q.get('pbk'), sid: q.get('sid'), spx: q.get('spx'), alpn: q.get('alpn'), encryption: q.get('encryption') || q.get('method'), insecure: q.get('allowInsecure') || q.get('insecure') });
  } catch { /* Keep the raw URI editable when it has a non-standard format. */ }
}
function encodeUtf8Base64(value) {
  const bytes = new TextEncoder().encode(value);
  let binary = '';
  bytes.forEach(byte => binary += String.fromCharCode(byte));
  return btoa(binary);
}
function buildManualUri(kind, name) {
  const host = $('#manual-host').value.trim();
  const port = $('#manual-port').value.trim();
  const user = $('#manual-user').value.trim();
  const secret = $('#manual-secret').value;
  const raw = $('#manual-uri').value.trim();
  if (kind === 'sing-box' || kind === 'mihomo') {
    const prefix = `${kind}://`;
    return raw.startsWith(prefix) ? raw : `${prefix}${raw}`;
  }
  if (kind === 'mhr') return 'mhr://local';
  const detail = {
    transport: $('#manual-transport').value,
    headerType: $('#manual-header-type').value,
    security: $('#manual-security').value,
    sni: $('#manual-sni').value.trim(),
    host: $('#manual-host-header').value.trim(),
    path: $('#manual-path').value.trim(),
    flow: $('#manual-flow').value.trim(),
    fp: $('#manual-fingerprint').value.trim(),
    pbk: $('#manual-public-key').value.trim(),
    sid: $('#manual-short-id').value.trim(),
    spx: $('#manual-spiderx').value.trim(),
    alpn: $('#manual-alpn').value.trim(),
    encryption: $('#manual-encryption').value.trim(),
    insecure: $('#manual-insecure').checked,
    wgAddress: $('#manual-wg-address').value.trim(),
    wgAllowed: $('#manual-wg-allowed').value.trim(),
    wgPsk: $('#manual-wg-psk').value.trim(),
    wgDns: $('#manual-wg-dns').value.trim(),
    wgMtu: $('#manual-wg-mtu').value.trim(),
    wgReserved: $('#manual-wg-reserved').value.trim()
  };
  const safeName = encodeURIComponent(name || kind);
  if (kind === 'aether') {
    const authority = host ? `${host.includes(':') ? `[${host}]` : host}${port ? `:${port}` : ''}` : '';
    const params = new URLSearchParams({
      protocol: $('#manual-aether-protocol').value,
      transport: $('#manual-aether-transport').value,
      scan: $('#manual-aether-scan').value,
      noize: $('#manual-aether-noize').value,
      ip: $('#aether-ip').value,
      sni: $('#manual-aether-sni').value.trim(),
      dns: $('#manual-aether-dns').value.trim()
    });
    params.set('ech', $('#aether-ech').checked ? '1' : '0');
    params.set('fragment', $('#aether-fragment').checked ? '1' : '0');
    return `aether://${authority}?${params.toString()}#${safeName}`;
  }
  const nativeProtocol = ({socks5:'socks', shadowsocks:'shadowsocks', hysteria2:'hysteria'})[kind] || kind;
  if (kind === 'wireguard' && host && port && !raw.startsWith('{')) {
    const endpointHost = host.includes(':') ? `[${host}]` : host;
    const wgSettings = {
      secretKey: secret,
      address: detail.wgAddress.split(',').map(x => x.trim()).filter(Boolean).length ? detail.wgAddress.split(',').map(x => x.trim()).filter(Boolean) : ['172.16.0.2/32'],
      mtu: Number(detail.wgMtu) || 1420,
      remoteDNS: detail.wgDns.split(',').map(x => x.trim()).filter(Boolean),
      peers: [{ publicKey: user, preSharedKey: detail.wgPsk, endpoint: `${endpointHost}:${port}`, allowedIPs: detail.wgAllowed.split(',').map(x => x.trim()).filter(Boolean) }]
    };
    const reserved = detail.wgReserved.split(',').map(x => Number(x.trim())).filter(Number.isFinite);
    if (reserved.length) wgSettings.reserved = reserved;
    return JSON.stringify({
      protocol: 'wireguard', tag: name || kind,
      settings: wgSettings
    }, null, 2);
  }
  if (raw.startsWith('{')) {
    try {
      const outbound = JSON.parse(raw);
      if (outbound.protocol || outbound.settings) {
        outbound.tag = name || kind;
        outbound.protocol = nativeProtocol;
        outbound.settings ||= {};
        if (host && kind !== 'wireguard') outbound.settings.address = host;
        if (port) outbound.settings.port = Number(port);
        if (kind === 'wireguard') {
          const peer = outbound.settings.peers?.[0];
          outbound.settings.secretKey = secret;
          if (host && outbound.settings.peers?.[0]) outbound.settings.peers[0].endpoint = `${host.includes(':') ? `[${host}]` : host}:${port || ''}`;
          if (peer) {
            peer.publicKey = user;
            if (detail.wgPsk) peer.preSharedKey = detail.wgPsk; else delete peer.preSharedKey;
            peer.allowedIPs = detail.wgAllowed.split(',').map(x => x.trim()).filter(Boolean);
          }
          const addresses = detail.wgAddress.split(',').map(x => x.trim()).filter(Boolean);
          outbound.settings.address = addresses.length ? addresses : ['172.16.0.2/32'];
          outbound.settings.remoteDNS = detail.wgDns.split(',').map(x => x.trim()).filter(Boolean);
          if (detail.wgMtu) outbound.settings.mtu = Number(detail.wgMtu); else delete outbound.settings.mtu;
          const reserved = detail.wgReserved.split(',').map(x => Number(x.trim())).filter(Number.isFinite);
          if (reserved.length) outbound.settings.reserved = reserved; else delete outbound.settings.reserved;
          return JSON.stringify(outbound, null, 2);
        }
        if (['vless', 'vmess'].includes(kind)) outbound.settings.id = user;
        if (kind === 'trojan') outbound.settings.password = secret || user;
        if (kind === 'hysteria2') outbound.settings.auth = secret || user;
        if (kind === 'shadowsocks') { outbound.settings.method = detail.encryption || user; outbound.settings.password = secret; }
        if (kind === 'socks5' || kind === 'http') { outbound.settings.user = user; outbound.settings.pass = secret; }
        if (['vless','vmess','trojan','shadowsocks'].includes(kind)) {
          outbound.streamSettings ||= {};
          outbound.streamSettings.method = detail.transport === 'tcp' ? 'raw' : detail.transport;
          if (detail.security) outbound.streamSettings.security = detail.security;
          else {
            delete outbound.streamSettings.security;
            delete outbound.streamSettings.tlsSettings;
            delete outbound.streamSettings.realitySettings;
          }
          if (detail.security === 'tls') outbound.streamSettings.tlsSettings = { ...(outbound.streamSettings.tlsSettings || {}), serverName: detail.sni || host, allowInsecure: detail.insecure, alpn: detail.alpn.split(',').map(x => x.trim()).filter(Boolean) };
          if (detail.security === 'reality') outbound.streamSettings.realitySettings = { ...(outbound.streamSettings.realitySettings || {}), serverName: detail.sni, fingerprint: detail.fp, publicKey: detail.pbk, shortId: detail.sid, spiderX: detail.spx };
          const method = outbound.streamSettings.method;
          if (['websocket','ws'].includes(method)) outbound.streamSettings.wsSettings = { ...(outbound.streamSettings.wsSettings || {}), path: detail.path, headers: { ...(outbound.streamSettings.wsSettings?.headers || {}), Host: detail.host } };
          if (method === 'grpc') outbound.streamSettings.grpcSettings = { ...(outbound.streamSettings.grpcSettings || {}), serviceName: detail.path, authority: detail.host };
          if (method === 'xhttp') outbound.streamSettings.xhttpSettings = { ...(outbound.streamSettings.xhttpSettings || {}), path: detail.path, host: detail.host };
          if (method === 'httpupgrade') outbound.streamSettings.httpupgradeSettings = { ...(outbound.streamSettings.httpupgradeSettings || {}), path: detail.path, host: detail.host };
        }
        if (kind === 'vless') { outbound.settings.encryption = detail.encryption || 'none'; outbound.settings.flow = detail.flow; }
        if (kind === 'hysteria2') { outbound.protocol = 'hysteria'; outbound.settings.version = 2; outbound.streamSettings.method = 'hysteria'; outbound.streamSettings.hysteriaSettings = { ...(outbound.streamSettings.hysteriaSettings || {}), version: 2, auth: secret || user }; }
        return JSON.stringify(outbound, null, 2);
      }
    } catch { return raw; }
    return raw;
  }
  if (!host || !port) return raw;
  if (kind === 'vmess') {
    let v = {};
    try { v = JSON.parse(decodeURIComponent(escape(atob(raw.replace(/^vmess:\/\//, '').split('#')[0])))); } catch {}
    Object.assign(v, { v: v.v || '2', ps: name || kind, add: host, port, id: user || v.id || '', aid: v.aid || '0', scy: detail.encryption || v.scy || 'auto', net: detail.transport === 'raw' ? 'tcp' : detail.transport, type: detail.headerType, tls: detail.security, sni: detail.sni, host: detail.host, path: detail.path, fp: detail.fp, spx: detail.spx, alpn: detail.alpn, insecure: detail.insecure ? '1' : '0' });
    return `vmess://${encodeUtf8Base64(JSON.stringify(v))}#${safeName}`;
  }
  if (kind === 'shadowsocks') return `ss://${encodeUtf8Base64(`${detail.encryption || user}:${secret}@${host}:${port}`)}#${safeName}`;
  if (['wireguard', 'ssh', 'chain', 'dnstt', 'masterdns', 'ping'].includes(kind)) return raw;
  const scheme = kind === 'socks5' ? 'socks5' : kind === 'hysteria2' ? 'hysteria2' : kind;
  let uri = `${scheme}://${host}:${port}`;
  if (kind === 'http' || kind === 'socks5') {
    uri = `${scheme}://${user ? `${encodeURIComponent(user)}:${encodeURIComponent(secret)}@` : ''}${host}:${port}#${safeName}`;
    return uri;
  }
  try {
    const u = raw ? new URL(raw) : new URL(`${scheme}://${host}:${port}`);
    u.hostname = host; u.port = port; u.hash = safeName;
    if (kind === 'vless') u.username = user;
    else if (kind === 'trojan' || kind === 'hysteria2') u.password = secret || user;
    const params = u.searchParams;
    const set = (key, value) => value ? params.set(key, value) : params.delete(key);
    set('type', detail.transport === 'tcp' ? 'tcp' : detail.transport);
    set('headerType', detail.headerType === 'none' ? '' : detail.headerType);
    set('security', detail.security);
    set('sni', detail.sni); set('host', detail.host); set('path', detail.path);
    set('flow', detail.flow); set('fp', detail.fp); set('pbk', detail.pbk); set('sid', detail.sid);
    set('spx', detail.spx); set('alpn', detail.alpn);
    if (kind === 'vless') set('encryption', detail.encryption || 'none');
    if (detail.insecure) params.set('insecure', '1'); else params.delete('insecure');
    if (kind === 'hysteria2' && !detail.security) params.set('security', 'tls');
    return u.toString();
  } catch { return raw || uri; }
}
function navigate(page) {
  const index = ['settings', 'logs', 'configs', 'home'].indexOf(page);
  $$('.page').forEach(panel => panel.classList.toggle('active', panel.id === page));
  $$('.dock button').forEach(button => button.classList.toggle('selected', button.dataset.page === page));
  $('.dock-indicator').style.transform = `translateX(${(3 - index) * 83}px)`;
  if (page === 'configs') render();
  if (page === 'logs') renderLogs();
  if (page === 'settings') refreshAetherIdentities();
}
async function startSelected() {
  if (!selected) { navigate('configs'); say(tr('noConfig')); return; }
  setState('connecting');
  try {
    const aetherOptions = {
      protocol: $('#aether-protocol').value, transport: $('#aether-transport').value,
      scan: $('#aether-scan').value, noize: $('#aether-noize').value,
      ip: $('#aether-ip').value, sni: $('#aether-sni').value,
      dns: $('#aether-dns').value, ech: $('#aether-ech').checked,
      fragment: $('#aether-fragment').checked, exit_loc: $('#aether-exit-loc').value,
      tor: $('#aether-tor').value, psiphon: $('#aether-psiphon').value,
      psiphon_mode: $('#aether-psiphon-mode').value, psiphon_region: $('#aether-psiphon-region').value,
      psiphon_cdn_ips: $('#aether-cdn-ips').value, psiphon_cdn_sni: $('#aether-cdn-sni').value,
      team: $('#aether-team').value, access_email: $('#aether-access-email').value,
      access_id: $('#aether-access-id').value, access_secret: $('#aether-access-secret').value,
      access_token: $('#aether-access-token').value
    };
    const secrets = await invoke('client_secrets');
    const mhrOptions = { script_id: secrets.mhr_script_id || '', auth_key: secrets.mhr_auth_key || '' };
    const sniOptions = { enabled: $('#sni-enabled').classList.contains('on'), fake_sni: $('#sni-fake-host').value.trim() || 'mci.ir', connect_ip: $('#sni-target-ip').value.trim(), connect_port: $('#sni-target-port').value.trim() };
    await invoke('start_core', { id: selected, mode, dns: $('#dns').value, dnsEnabled, route: $('#route').value, shareLan, aetherOptions, mhrOptions, sniOptions });
    setState('connected');
  } catch (error) { setState('disconnected'); say(String(error).slice(0, 180)); }
}
async function stopCore() {
  try { await invoke('stop_core'); } catch {}
  setState('disconnected');
}
async function choose(id) {
  const restart = state === 'connected';
  if (restart) await stopCore();
  selected = id;
  render();
  navigate('home');
  if (restart) await startSelected();
}
async function toggleConnection() {
  if (state === 'connecting') return;
  if (state === 'connected') return stopCore();
  return startSelected();
}
function applyLanguage() {
  document.documentElement.lang = lang;
  document.documentElement.dir = lang === 'fa' ? 'rtl' : 'ltr';
  const currentPage = $('.page.active')?.id || 'home';
  $('#configs .eyebrow').textContent = tr('library');
  $('#settings .eyebrow').textContent = tr('preferences');
  $('#configs h1').textContent = tr('configs');
  $('#settings h1').textContent = tr('settings');
  $('#logs-title').textContent = tr('logs');
  $$('.dock button').forEach(button => button.querySelector('span').textContent = tr(button.dataset.page));
  $('#search').placeholder = tr('search');
  $('#empty b').textContent = tr('empty');
  $('#empty-add').textContent = tr('add');
  $('#core-label').textContent = tr('core');
  $('#update').textContent = tr('update');
  $('#dns-title').textContent = tr('dns');
  $('#route-title').textContent = tr('route');
  $('#route-note').textContent = tr('iran');
  $('#dns-toggle').setAttribute('aria-pressed', String(dnsEnabled));
  $('#dns-toggle').classList.toggle('on', dnsEnabled);
  $('#share-lan').setAttribute('aria-pressed', String(shareLan));
  $('#share-lan').classList.toggle('on', shareLan);
  $('#speed-title').textContent = tr('speed');
  $('#clear-title').textContent = tr('clear');
  $('#clear-note').textContent = tr('clearNote');
  $('#sub-name-label').textContent = tr('name');
  $('#sub-url-label').textContent = tr('link');
  $('#manual-name-label').textContent = tr('name');
  $('#manual-uri-label').textContent = tr('uri');
  $('#manual-kind-label').textContent = tr('protocol');
  $('#clipboard').textContent = tr('clipboard');
  $('#scan-qr').textContent = lang === 'fa' ? 'اسکن QR تصویر' : 'Scan QR image';
  $('#import').textContent = tr('addNow');
  $('#save-sub').textContent = tr('save');
  $('#save-manual').textContent = tr('save');
  $('#modal-title').textContent = tr(editingId ? 'edit' : 'add');
  $$('.modal-tabs button').forEach(button => button.textContent = tr(button.dataset.tab));
  $('#profile-title').textContent = selected ? $('#profile-title').textContent : tr('choose');
  $('#status').textContent = tr(state === 'connecting' ? 'connecting' : state === 'connected' ? 'connected' : state === 'disconnected' ? 'disconnected' : 'ready');
  if (currentPage === 'configs') render();
}

$('#dock').addEventListener('click', e => { const button = e.target.closest('[data-page]'); if (button) navigate(button.dataset.page); });
$('#logs-live').onclick = () => { liveLogs = !liveLogs; localStorage.liveLogs = String(liveLogs); renderLogs(); };
$('#logs-clear').onclick = () => { logEntries.length = 0; renderLogs(); };
$('#aether-identity-refresh').onclick = refreshAetherIdentities;
$('#aether-register').onclick = async () => {
  const button = $('#aether-register');
  button.disabled = true;
  try {
    await invoke('aether_register', { kind: $('#aether-register-kind').value });
    say(lang === 'fa' ? 'ثبت identity تمام شد' : 'Identity registration complete');
    await refreshAetherIdentities();
    navigate('logs');
  } catch (error) { say(String(error).slice(0, 180)); }
  finally { button.disabled = false; }
};
$('#connect').onclick = toggleConnection;
$('#select-active').onclick = () => navigate('configs');
$('#add').onclick = $('#empty-add').onclick = () => openModal();
$('#close-modal').onclick = () => $('#modal').classList.remove('open');
$('#modal').onclick = e => { if (e.target.id === 'modal') $('#modal').classList.remove('open'); };
$$('.modal-tabs button').forEach(button => button.onclick = () => switchTab(button.dataset.tab));
$('#import').onclick = async () => {
  const value = $('#paste-input').value.trim();
  if (/^https?:\/\//i.test(value) && !/^(http|https|socks5?):\/\/[^\s]+@/i.test(value)) {
    const name = new URL(value).hostname || tr('subscription');
    try { const sub = await invoke('add_subscription', { name, url: value, userAgent: '', moreUrls: '' }); await invoke('refresh_subscriptions', { mode: 'auto', only: sub.id }); await load(); $('#modal').classList.remove('open'); say(tr('saved')); }
    catch (error) { say(String(error)); }
    return;
  }
  try {
    const added = await invoke('import_text', { text: value });
    const fresh = added.filter(item => !data.profiles.some(profile => profile.uri === item.uri));
    await load();
    $('#modal').classList.remove('open');
    $('#paste-input').value = '';
    say(`${fresh.length} ${lang === 'fa' ? 'افزوده شد' : 'added'}`);
  } catch (error) { say(String(error)); }
};
$('#clipboard').onclick = async () => { try { $('#paste-input').value = await navigator.clipboard.readText(); } catch { say(tr('clipError')); } };
$('#scan-qr').onclick = () => $('#qr-file').click();
$('#qr-file').onchange = async event => {
  const file = event.target.files?.[0];
  if (!file) return;
  try {
    if (!('BarcodeDetector' in window)) throw new Error(lang === 'fa' ? 'خواندن QR در این نسخه پشتیبانی نمی‌شود' : 'QR scanning is unsupported in this version');
    const detector = new BarcodeDetector({ formats: ['qr_code'] });
    const bitmap = await createImageBitmap(file);
    try {
      const codes = await detector.detect(bitmap);
      if (!codes.length || !codes[0].rawValue) throw new Error(lang === 'fa' ? 'QR پیدا نشد' : 'No QR code found');
      $('#paste-input').value = codes.map(code => code.rawValue).join('\n');
      say(lang === 'fa' ? 'QR خوانده شد' : 'QR scanned');
    } finally { bitmap.close(); }
  } catch (error) { say(String(error)); }
  event.target.value = '';
};
const kinds = ['socks5', 'http', 'wireguard', 'vless', 'vmess', 'ssh', 'chain', 'trojan', 'shadowsocks', 'hysteria2', 'tuic', 'sing-box', 'mihomo', 'aether', 'mhr', 'ping', 'dnstt', 'masterdns'];
$('#manual-kind').innerHTML = kinds.map(kind => `<option value="${kind}">${kind.toUpperCase()}</option>`).join('');
['#manual-host', '#manual-port', '#manual-user', '#manual-secret', '#manual-transport', '#manual-header-type', '#manual-security', '#manual-sni', '#manual-host-header', '#manual-path', '#manual-flow', '#manual-fingerprint', '#manual-public-key', '#manual-short-id', '#manual-spiderx', '#manual-alpn', '#manual-encryption', '#manual-insecure', '#manual-wg-address', '#manual-wg-allowed', '#manual-wg-psk', '#manual-wg-dns', '#manual-wg-mtu', '#manual-wg-reserved'].forEach(selector => $(selector).addEventListener('input', () => { manualDirty = true; }));
$('#manual-kind').onchange = () => { manualDirty = true; updateManualFields(); };
['#manual-aether-protocol','#manual-aether-transport','#manual-aether-scan','#manual-aether-noize','#manual-aether-sni','#manual-aether-dns'].forEach(selector => $(selector).addEventListener('change', () => { manualDirty = true; }));
['#manual-aether-sni','#manual-aether-dns'].forEach(selector => $(selector).addEventListener('input', () => { manualDirty = true; }));
$('#save-manual').onclick = async () => {
  const name = $('#manual-name').value || $('#manual-kind').value;
  const kind = $('#manual-kind').value;
  const args = { name, kind, uri: manualDirty ? buildManualUri(kind, name) : $('#manual-uri').value.trim() };
  if (!args.uri) { say(tr('uri')); return; }
  try {
    if (editingId) await invoke('update_profile', { id: editingId, ...args });
    else await invoke('save_profile', args);
    await load();
    $('#modal').classList.remove('open');
    say(tr('saved'));
  } catch (error) { say(String(error)); }
};
$('#save-sub').onclick = async () => {
  const name = $('#sub-name').value.trim();
  const url = $('#sub-url').value.trim();
  if (!name || !/^https?:\/\//i.test(url)) { say('URL'); return; }
  try {
    const sub = await invoke('add_subscription', {
      name, url,
      userAgent: $('#sub-user-agent').value.trim(),
      moreUrls: $('#sub-more-urls').value.trim()
    });
    await invoke('refresh_subscriptions', { mode: 'auto', only: sub.id });
    await load(); $('#modal').classList.remove('open'); say(tr('saved'));
  }
  catch (error) { say(String(error)); }
};
$('#profile-list').onclick = async e => {
  const button = e.target.closest('[data-act]');
  if (!button) return;
  const row = button.closest('[data-id]');
  const profile = data.profiles.find(p => p.id === row.dataset.id);
  if (button.dataset.act === 'use') return choose(profile.id);
  if (button.dataset.act === 'edit') return openEditor(profile);
  if (button.dataset.act === 'delete') {
    await invoke('delete_profile', { id: profile.id });
    if (selected === profile.id) selected = null;
    await load();
    say(tr('deleted'));
  }
  if (button.dataset.act === 'copy') { try { await navigator.clipboard.writeText(profile.uri); say(tr('copied')); } catch { say(tr('clipError')); } }
};
$('#search').oninput = render;
async function refreshSubs(modeName, only = null) {
  try { const count = await invoke('refresh_subscriptions', { mode: modeName, only }); await load(); $('#refresh-menu').classList.remove('open'); say(`${count} ${lang === 'fa' ? 'به‌روز شد' : 'updated'}`); }
  catch (error) { $('#refresh-menu').classList.remove('open'); say(String(error)); }
}
$('#refresh').onclick = e => { e.stopPropagation(); $('#refresh-menu').classList.toggle('open'); };
$('#refresh-menu').onclick = e => {
  const button = e.target.closest('[data-refresh]'); if (!button) return;
  const [scope, selectedMode] = button.dataset.refresh.startsWith('current-') ? ['current', button.dataset.refresh.slice(8)] : ['all', button.dataset.refresh];
  const current = data.profiles.find(profile => profile.id === selected)?.source;
  if (scope === 'current' && !data.subs.some(sub => sub.id === current)) { say(lang === 'fa' ? 'کانفیگ ساب انتخاب نشده' : 'Select a subscription config first'); return; }
  refreshSubs(selectedMode, scope === 'current' ? current : null);
};
window.addEventListener('click', e => { if (!e.target.closest('.refresh-wrap')) $('#refresh-menu').classList.remove('open'); });
$('#ping').onclick = async () => {
  try {
    const results = await invoke('ping_all', { provider: $('#ping-provider').value });
    const values = new Map(results);
    data.profiles.forEach(profile => { profile.latency = values.get(profile.id) ?? null; });
    render();
    say(tr('pingDone'));
  } catch (error) { say(String(error)); }
};
$('#clear').onclick = async () => {
  if (!confirm(tr('confirmClear'))) return;
  await invoke('clear_all'); data = { profiles: [], subs: [] }; selected = null; await load(); say(tr('cleared'));
};
$('#clear-stats').onclick = async () => { await invoke('clear_stats'); await load(); say(lang === 'fa' ? 'آمار پاک شد' : 'Statistics cleared'); };
$('#backup-export').onclick = async () => {
  const blob = new Blob([await invoke('export_backup')], { type: 'application/json' });
  const url = URL.createObjectURL(blob); const link = document.createElement('a'); link.href = url; link.download = 'xray-glass-backup.json'; link.click(); URL.revokeObjectURL(url);
};
$('#backup-import').onclick = () => $('#backup-file').click();
$('#backup-file').onchange = async event => {
  const file = event.target.files[0]; if (!file) return;
  try { await invoke('import_backup', { text: await file.text() }); selected = null; await load(); say(lang === 'fa' ? 'بازیابی شد' : 'Backup restored'); }
  catch (error) { say(String(error)); }
  event.target.value = '';
};
$('#open-storage').onclick = async () => { try { await invoke('open_storage_location'); } catch (error) { say(String(error)); } };
$('#update').onclick = async () => { try { say(`${tr('updateFound')} ${await invoke('check_update')}`); } catch (error) { say(String(error)); } };
$('#speed').onclick = async () => { try { const mbps = await invoke('speed_test'); $('#speed-result').textContent = `${mbps} Mbps`; say(`${mbps} Mbps`); } catch (error) { say(String(error)); } };
$('#mode').onclick = async () => {
  const restart = state === 'connected';
  if (restart) await stopCore();
  $('#mode').classList.toggle('expanded');
  mode = mode === 'proxy' ? 'tun' : 'proxy';
  $$('#mode span').forEach((item, i) => item.classList.toggle('active', mode === 'proxy' ? i === 0 : i === 1));
  $('#mode-label').textContent = mode === 'proxy' ? 'P · PROXY' : 'T · TUN';
  if (mode === 'tun') say(tr('tunPermission'));
  if (restart) await startSelected();
};
$('#theme').onclick = () => { theme = theme === 'dark' ? 'light' : 'dark'; document.body.classList.toggle('light', theme === 'light'); localStorage.theme = theme; wave(); };
$('#lang').onclick = () => { lang = lang === 'fa' ? 'en' : 'fa'; localStorage.lang = lang; applyLanguage(); wave(); };
async function applyNetworkSettings() {
  localStorage.dnsEnabled = String(dnsEnabled); localStorage.shareLan = String(shareLan);
  if (state === 'connected') { await stopCore(); await startSelected(); }
}
$('#dns-toggle').onclick = () => { dnsEnabled = !dnsEnabled; applyLanguage(); applyNetworkSettings(); };
$('#share-lan').onclick = () => { shareLan = !shareLan; applyLanguage(); applyNetworkSettings(); };
$('#dns-preset').onchange = () => { $('#dns').value = $('#dns-preset').value === 'localhost' ? 'localhost' : $('#dns-preset').value; localStorage.dns = $('#dns').value; if (state === 'connected') applyNetworkSettings(); };
$('#dns').onchange = () => { localStorage.dns = $('#dns').value; if (state === 'connected') applyNetworkSettings(); };
$('#route').onchange = () => { localStorage.route = $('#route').value; if (state === 'connected') applyNetworkSettings(); };
['#aether-protocol','#aether-transport','#aether-scan','#aether-noize','#aether-ip','#aether-sni','#aether-dns','#aether-ech','#aether-fragment'].forEach(selector => $(selector).addEventListener('change', async () => {
  saveAetherSettings();
  if (state === 'connected' && data.profiles.find(profile => profile.id === selected)?.kind === 'aether') {
    await stopCore(); await startSelected();
  }
}));
$('#aether-scan-now').onclick = async () => {
  if (!selected) { say(tr('noConfig')); return; }
  const button = $('#aether-scan-now'); button.disabled = true;
  try {
    const aetherOptions = {
      protocol: $('#aether-protocol').value, transport: $('#aether-transport').value,
      scan: $('#aether-scan').value, noize: $('#aether-noize').value,
      ip: $('#aether-ip').value, sni: $('#aether-sni').value, dns: $('#aether-dns').value,
      ech: $('#aether-ech').checked, fragment: $('#aether-fragment').checked
    };
    const ms = await invoke('aether_scan', { id: selected, aetherOptions });
    say(`${lang === 'fa' ? 'اسکن Aether' : 'Aether scan'} · ${ms} ms`);
  } catch (error) { say(String(error)); }
  finally { button.disabled = false; }
};

let cursorX = -20, cursorY = -20, targetX = -20, targetY = -20, cursorFrame = 0;
function moveCursor() {
  cursorX += (targetX - cursorX) * .28;
  cursorY += (targetY - cursorY) * .28;
  const cursor = $('.cursor-glow');
  cursor.style.left = `${cursorX}px`; cursor.style.top = `${cursorY}px`;
  if (Math.abs(targetX - cursorX) + Math.abs(targetY - cursorY) > .2) cursorFrame = requestAnimationFrame(moveCursor);
  else cursorFrame = 0;
}
window.addEventListener('pointermove', event => {
  targetX = event.clientX; targetY = event.clientY;
  const cursor = $('.cursor-glow'); cursor.classList.add('visible');
  cursor.classList.toggle('hovering', Boolean(event.target.closest('button, a, [role="button"]')));
  document.documentElement.style.setProperty('--pointer-x', `${targetX}px`);
  document.documentElement.style.setProperty('--pointer-y', `${targetY}px`);
  if (!cursorFrame) cursorFrame = requestAnimationFrame(moveCursor);
}, { passive: true });
window.addEventListener('pointerleave', () => $('.cursor-glow').classList.remove('visible'));
window.addEventListener('scroll', () => {
  document.body.classList.add('is-scrolling');
  clearTimeout(window.scrollTimer);
  window.scrollTimer = setTimeout(() => document.body.classList.remove('is-scrolling'), 160);
}, { passive: true });
$('#mode').querySelectorAll('span').forEach((item, i) => item.classList.toggle('active', i === 0));
setInterval(async () => {
  if (state !== 'connected') return;
  $('#uptime').textContent = new Date(Date.now() - started).toISOString().slice(14, 19);
  try { if (!await invoke('core_running')) { setState('disconnected'); say(tr('disconnected')); } } catch {}
}, 1000);
if (theme === 'light') document.body.classList.add('light');
$('#dns').value = localStorage.dns || $('#dns').value;
$('#route').value = localStorage.route || 'ir';
applyLanguage();
load();
