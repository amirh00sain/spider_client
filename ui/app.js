const invoke = (...args) => window.__TAURI__.core.invoke(...args);
const $ = s => document.querySelector(s);
const $$ = s => [...document.querySelectorAll(s)];
const text = {
  fa: { ready:'آماده', connecting:'در حال اتصال', connected:'متصل', disconnected:'قطع', none:'بدون کانفیگ', choose:'انتخاب کانفیگ', select:'برای شروع کانفیگ انتخاب کنید', configs:'کانفیگ‌ها', settings:'تنظیمات', home:'خانه', search:'جستجو', empty:'کانفیگی نیست', add:'افزودن', paste:'کلیپ‌بورد', sub:'ساب', manual:'دستی', library:'کتابخانه', preferences:'تنظیمات', core:'هسته', version:'نسخه Xray', update:'بررسی', dns:'DNS', route:'مسیر', iran:'دامنه‌های ایران مستقیم', speed:'تست سرعت', clear:'حذف همه', clearNote:'کانفیگ‌ها و ساب‌ها', subscription:'Subscription', uri:'URI · JSON · Base64', name:'نام', link:'پیوند', protocol:'پروتکل', save:'ذخیره', clipboard:'از کلیپ‌بورد', addNow:'افزودن', connect:'اتصال', copy:'کپی', edit:'ویرایش', remove:'حذف', pingDone:'پینگ انجام شد', saved:'ذخیره شد', deleted:'حذف شد', copied:'کپی شد', updateFound:'آخرین نسخه Xray', noConfig:'ابتدا کانفیگ انتخاب کنید', tunPermission:'TUN به مجوز شبکه نیاز دارد', cleared:'پاک شد', confirmClear:'همه کانفیگ‌ها و ساب‌ها حذف شوند؟', clipError:'کلیپ‌بورد در دسترس نیست', direct:'مستقیم', global:'سراسری' },
  en: { ready:'READY', connecting:'CONNECTING', connected:'CONNECTED', disconnected:'DISCONNECTED', none:'No config', choose:'Select config', select:'Choose a config to start', configs:'Configs', settings:'Settings', home:'Home', search:'Search', empty:'No configs', add:'Add', paste:'Clipboard', sub:'Sub', manual:'Manual', library:'LIBRARY', preferences:'PREFERENCES', core:'Core', version:'Xray version', update:'Check', dns:'DNS', route:'Route', iran:'Iran domains direct', speed:'Speed test', clear:'Clear all', clearNote:'Configs and subscriptions', subscription:'Subscription', uri:'URI · JSON · Base64', name:'Name', link:'Link', protocol:'Protocol', save:'Save', clipboard:'Paste clipboard', addNow:'Add', connect:'Connect', copy:'Copy', edit:'Edit', remove:'Delete', pingDone:'Ping complete', saved:'Saved', deleted:'Deleted', copied:'Copied', updateFound:'Latest Xray', noConfig:'Select a config first', tunPermission:'TUN needs network permission', cleared:'Cleared', confirmClear:'Delete all configs and subscriptions?', clipError:'Clipboard unavailable', direct:'Direct', global:'Global' }
};
let lang = localStorage.lang || 'fa';
let theme = localStorage.theme || 'dark';
let mode = 'proxy';
let state = 'disconnected';
let data = { profiles: [], subs: [] };
let selected = null;
let editingId = null;
let manualDirty = false;
let started = 0;
let toastTimer;
const tr = key => text[lang][key] || key;

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
  manualDirty = false;
  switchTab(tab);
  $('#modal').classList.add('open');
}
function openEditor(profile) {
  editingId = profile.id;
  $('#modal-title').textContent = tr('edit');
  $('#manual-name').value = profile.name;
  $('#manual-uri').value = profile.uri;
  $('#manual-kind').value = profile.kind;
  fillBasicFields(profile);
  manualDirty = false;
  switchTab('manual');
  $('#modal').classList.add('open');
}
function setBasicFields(host, port, user, secret) {
  $('#manual-host').value = host || '';
  $('#manual-port').value = port || '';
  $('#manual-user').value = user || '';
  $('#manual-secret').value = secret || '';
}
function fillBasicFields(profile) {
  setBasicFields('', '', '', '');
  try {
    if (profile.uri.startsWith('vmess://')) {
      const raw = decodeURIComponent(escape(atob(profile.uri.slice(8).split('#')[0])));
      const v = JSON.parse(raw);
      setBasicFields(v.add, v.port, v.id, v.id);
      return;
    }
    if (profile.uri.trimStart().startsWith('{')) {
      const v = JSON.parse(profile.uri), s = v.settings || {};
      const peer = s.peers?.[0];
      const host = s.address || s.server || s.vnext?.[0]?.address || peer?.endpoint?.split(':').slice(0,-1).join(':');
      const port = s.port || s.vnext?.[0]?.port || peer?.endpoint?.split(':').at(-1);
      const user = s.id || s.vnext?.[0]?.users?.[0]?.id;
      const secret = s.password || s.secretKey || s.vnext?.[0]?.users?.[0]?.password;
      setBasicFields(host, port, user, secret);
      return;
    }
    if (profile.uri.startsWith('ss://')) {
      const raw = profile.uri.slice(5).split('#')[0];
      const decoded = atob(raw.replace(/-/g,'+').replace(/_/g,'/'));
      const [auth, endpoint] = decoded.split('@');
      const [method, password] = auth.split(':');
      const [host, port] = endpoint.split(':');
      setBasicFields(host, port, method, password);
      return;
    }
    const u = new URL(profile.uri);
    const host = u.hostname;
    const port = u.port;
    const user = decodeURIComponent(u.username || '');
    const secret = decodeURIComponent(u.password || '');
    setBasicFields(host, port, user, secret);
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
  if (!host || !port) return $('#manual-uri').value.trim();
  const safeName = encodeURIComponent(name || kind);
  const raw = $('#manual-uri').value.trim();
  // Keep transport and TLS/Reality parameters when an existing link is edited.
  if (raw && ['vless', 'trojan', 'hysteria2', 'hysteria'].includes(kind)) {
    try {
      const u = new URL(raw);
      if (u.protocol.slice(0, -1) === kind || (kind === 'hysteria2' && u.protocol === 'hy2:')) {
        u.hostname = host;
        u.port = port;
        if (kind === 'vless') u.username = user;
        else u.password = secret || user;
        u.hash = safeName;
        return u.toString();
      }
    } catch { /* Fall back to a basic link if the pasted URI is incomplete. */ }
  }
  if (kind === 'vmess') {
    let v = {};
    try { v = JSON.parse(decodeURIComponent(escape(atob(raw.replace(/^vmess:\/\//,'' ).split('#')[0])))); } catch {}
    Object.assign(v, { v: v.v || '2', ps: name || kind, add: host, port, id: user || v.id || '', aid: v.aid || '0', scy: v.scy || 'auto', net: v.net || 'tcp' });
    return `vmess://${encodeUtf8Base64(JSON.stringify(v))}#${safeName}`;
  }
  if (kind === 'wireguard' || kind === 'ssh' || kind === 'chain' || kind === 'dnstt' || kind === 'masterdns' || kind === 'ping') return raw;
  if (kind === 'shadowsocks') {
    if (user && secret) return `ss://${encodeUtf8Base64(`${user}:${secret}@${host}:${port}`)}#${safeName}`;
    return raw;
  }
  const scheme = kind === 'socks5' ? 'socks5' : kind === 'hysteria2' ? 'hysteria2' : kind;
  let uri = `${scheme}://${host}:${port}`;
  if (kind === 'vless') uri = `vless://${encodeURIComponent(user)}@${host}:${port}?encryption=none&type=tcp#${safeName}`;
  else if (kind === 'trojan') uri = `trojan://${encodeURIComponent(secret || user)}@${host}:${port}?security=tls&sni=${encodeURIComponent(host)}#${safeName}`;
  else if (kind === 'hysteria2') uri = `hysteria2://${encodeURIComponent(secret || user)}@${host}:${port}/?sni=${encodeURIComponent(host)}#${safeName}`;
  else if (kind === 'http' || kind === 'socks5') uri = `${scheme}://${user ? `${encodeURIComponent(user)}:${encodeURIComponent(secret)}@` : ''}${host}:${port}#${safeName}`;
  else if (raw) {
    try {
      const u = new URL(raw);
      u.hostname = host; u.port = port;
      if (kind === 'vless') u.username = encodeURIComponent(user);
      if (kind === 'trojan' || kind === 'hysteria2') u.password = encodeURIComponent(secret || user);
      u.hash = safeName;
      uri = u.toString();
    } catch {}
  }
  return uri;
}
function navigate(page) {
  const index = ['settings', 'configs', 'home'].indexOf(page);
  $$('.page').forEach(panel => panel.classList.toggle('active', panel.id === page));
  $$('.dock button').forEach(button => button.classList.toggle('selected', button.dataset.page === page));
  $('.dock-indicator').style.transform = `translateX(${(2 - index) * 92}px)`;
  if (page === 'configs') render();
}
async function startSelected() {
  if (!selected) { navigate('configs'); say(tr('noConfig')); return; }
  setState('connecting');
  try {
    await invoke('start_core', { id: selected, mode, dns: $('#dns').value, route: $('#route').value });
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
  $$('.dock button').forEach(button => button.querySelector('span').textContent = tr(button.dataset.page));
  $('#search').placeholder = tr('search');
  $('#empty b').textContent = tr('empty');
  $('#empty-add').textContent = tr('add');
  $('#core-label').textContent = tr('core');
  $('#update').textContent = tr('update');
  $('#dns-title').textContent = tr('dns');
  $('#route-title').textContent = tr('route');
  $('#route-note').textContent = tr('iran');
  $('#speed-title').textContent = tr('speed');
  $('#clear-title').textContent = tr('clear');
  $('#clear-note').textContent = tr('clearNote');
  $('#sub-name-label').textContent = tr('name');
  $('#sub-url-label').textContent = tr('link');
  $('#manual-name-label').textContent = tr('name');
  $('#manual-uri-label').textContent = tr('uri');
  $('#manual-kind-label').textContent = tr('protocol');
  $('#clipboard').textContent = tr('clipboard');
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
    try { await invoke('add_subscription', { name, url: value }); await invoke('refresh_subscriptions'); await load(); $('#modal').classList.remove('open'); say(tr('saved')); }
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
const kinds = ['socks5', 'http', 'wireguard', 'vless', 'vmess', 'ssh', 'chain', 'trojan', 'shadowsocks', 'ping', 'hysteria2', 'dnstt', 'masterdns'];
$('#manual-kind').innerHTML = kinds.map(kind => `<option value="${kind}">${kind.toUpperCase()}</option>`).join('');
['#manual-host', '#manual-port', '#manual-user', '#manual-secret'].forEach(selector => $(selector).addEventListener('input', () => { manualDirty = true; }));
$('#manual-kind').onchange = () => { manualDirty = true; };
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
  try { await invoke('add_subscription', { name, url }); await invoke('refresh_subscriptions'); await load(); $('#modal').classList.remove('open'); say(tr('saved')); }
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
$('#refresh').onclick = async () => { try { const count = await invoke('refresh_subscriptions'); await load(); say(`${count} ${lang === 'fa' ? 'به‌روز شد' : 'updated'}`); } catch (error) { say(String(error)); } };
$('#ping').onclick = async () => {
  try {
    const results = await invoke('ping_all');
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
$('#dns').onchange = $('#route').onchange = async () => { if (state === 'connected') { await stopCore(); await startSelected(); } };

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
applyLanguage();
load();
