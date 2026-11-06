/* SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE */
'use strict';
/* Pomme — interface. JavaScript sans framework : listes virtuelles, aucune
   boucle d'animation au repos, images chargées à la taille affichée. */

// ── outils ───────────────────────────────────────────────────────────────
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const native = (cmd, data = {}) => window.ipc && window.ipc.postMessage(JSON.stringify({ cmd, ...data }));
const log = msg => native('log', { msg: String(msg) });
/** Interruption normale d'une lecture par une autre (clics rapprochés) : pas une erreur. */
const isAbort = e => {
  if (!e) return false;
  // les erreurs de MusicKit (MKError) portent le texte dans description ou errorCode
  const s = [e.name, e.message, e.description, e.errorCode, e.reason?.message, String(e)].filter(Boolean).join(' ');
  return /AbortError|interrupt|abort|play\(\)|pause\(\)/i.test(s);
};
/** File de secours après « One or more items could not be resolved » :
 *  sans les identifiants cités par l'erreur, sinon avec le seul morceau choisi. */
function unresolved(desc, msg, attempt) {
  const ids = desc.songs;
  if (!Array.isArray(ids) || ids.length < 2) return null;
  const start = ids[desc.startWith] ?? ids[0];
  let keep = ids.filter(id => !msg.includes(String(id)));
  if (attempt > 0 || keep.length === ids.length || !keep.length) keep = [start];
  const i = Math.max(0, keep.indexOf(start));
  return { songs: keep, startWith: i, startPosition: i };
}
addEventListener('error', e => log('ERREUR JS : ' + e.message + ' @' + e.lineno));
addEventListener('unhandledrejection', e => { if (isAbort(e.reason)) { e.preventDefault(); return; } log('PROMESSE : ' + (e.reason?.message || e.reason)); });
const store = {
  get(k, d) { try { const v = localStorage.getItem('pomme.' + k); return v === null ? d : JSON.parse(v); } catch { return d; } },
  set(k, v) { try { localStorage.setItem('pomme.' + k, JSON.stringify(v)); } catch { } },
};
const fmtTime = s => { s = Math.max(0, Math.floor(s || 0)); const h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60; return (h ? h + ':' + String(m).padStart(2, '0') : m) + ':' + String(s % 60).padStart(2, '0'); };
const dpr = () => Math.min(2, window.devicePixelRatio || 1);
/** Pochette Apple à la taille affichée ({w}x{h}{c}.{f} → 96x96bb.webp). */
const artUrl = (art, size) => {
  if (!art) return '';
  if (typeof art === 'string') return art.includes('{w}') ? artUrl({ url: art }, size) : art;
  const px = Math.round(size * dpr());
  return (art.url || '').replace('{w}', px).replace('{h}', px).replace('{c}', 'bb').replace('{f}', 'webp');
};
const img = (src, cls = '') => src ? `<img class="${cls}" loading="lazy" decoding="async" src="${esc(src)}" alt="">` : '';
const PL_ICON = '<svg viewBox="0 0 24 24"><path fill="currentColor" d="M10 4v10.55A4 4 0 1 0 12 18V8h6V4z"/></svg>';
/** Vignette d'artiste : photo du catalogue, initiale en dessous si absente. */
const artistArt = (name, art) => `<div class="art letter">${img(artUrl(art, 160))}<span>${esc((name || '?')[0])}</span></div>`;
// image introuvable : on retire la balise, la vignette colorée reste visible
addEventListener('error', e => { const t = e.target; if (t.tagName === 'IMG' && !t.id) t.remove(); }, true);
const ICON_PLAY = '<path d="M8 5v14l11-7z"/>';
const ICON_PAUSE = '<path d="M7 5h4v14H7zm6 0h4v14h-4z"/>';
let toastTimer = 0;
function toast(msg) {
  const t = $('#toast'); t.textContent = trad(String(msg)); t.hidden = false;
  clearTimeout(toastTimer); toastTimer = setTimeout(() => (t.hidden = true), 3200);
}

// ── Apple Music ─────────────────────────────────────────────────────────
/** Message d'erreur du moteur natif (« Fichier illisible : détail ») : traduit, détail conservé. */
const tradErr = m => { m = String(m || ''); const i = m.indexOf(' : '); return i > 0 && trad(m) === m ? trad(m.slice(0, i)) + trad(' : ') + m.slice(i + 3) : trad(m); };
const apple = { music: null, token: null, sf: 'fr', ok: false, authorized: false, cache: new Map(), playlists: [] };

/** Clé du lecteur web d'Apple : relue sur music.apple.com puis gardée tant
 *  qu'elle est valable (plusieurs semaines). */
async function developerToken(force) {
  const cached = store.get('devtoken', null);
  const now = Date.now() / 1000;
  if (!force && cached && cached.exp - now > 3 * 86400) return cached.token;
  const html = await (await fetch('/', { credentials: 'include' })).text();
  const scripts = [...html.matchAll(/src="(\/assets\/index[^"]+\.js)"/g)].map(m => m[1]);
  for (const src of scripts) {
    const js = await (await fetch(src)).text();
    for (const m of js.matchAll(/eyJ[\w-]+\.[\w-]+\.[\w-]+/g)) {
      try {
        const p = JSON.parse(atob(m[0].split('.')[1].replace(/-/g, '+').replace(/_/g, '/')));
        if (p.iss && p.exp > now) { store.set('devtoken', { token: m[0], exp: p.exp }); return m[0]; }
      } catch { }
    }
  }
  throw new Error('clé introuvable');
}

function loadScript(src) {
  return new Promise((ok, ko) => { const s = document.createElement('script'); s.src = src; s.async = true; s.onload = ok; s.onerror = ko; document.head.appendChild(s); });
}

function bitrate() {
  const q = String(store.get('quality', 'auto'));
  if (q === 'auto') { const c = navigator.connection; return c && (c.saveData || /2g|3g/.test(c.effectiveType || '')) ? 64 : 256; }
  return +q || 256;
}
function applyBitrate() {
  try { apple.music.bitrate = bitrate() >= 256 ? MusicKit.PlaybackBitrate.HIGH : MusicKit.PlaybackBitrate.STANDARD; } catch { }
}

async function initApple() {
  try {
    apple.token = await developerToken();
    if (!window.MusicKit?.configure) {
      const loaded = new Promise(r => document.addEventListener('musickitloaded', r, { once: true }));
      await loadScript('https://js-cdn.music.apple.com/musickit/v3/musickit.js');
      if (!window.MusicKit?.configure) await loaded;
    }
    apple.music = await MusicKit.configure({ developerToken: apple.token, app: { name: 'Pomme', build: app.version || '1.0' } });
    apple.ok = true;
    log('MusicKit prêt, connecté=' + apple.music.isAuthorized);
    const m = apple.music;
    applyBitrate();
    m.volume = player.volume;
    m.shuffleMode = player.shuffle ? 1 : 0;
    m.addEventListener('playbackStateDidChange', () => player.src === 'apple' && player.onState());
    m.addEventListener('nowPlayingItemDidChange', () => player.src === 'apple' && player.onTrack());
    m.addEventListener('playbackTimeDidChange', () => player.src === 'apple' && player.onTime());
    m.addEventListener('queueItemsDidChange', queueChanged);
    m.addEventListener('authorizationStatusDidChange', onAuth);
    await onAuth();
  } catch (e) {
    log('Apple KO : ' + (e.message || e));
    apple.ok = false;
    document.body.classList.add('signed-out');
    toast('Apple Music est indisponible pour le moment : la musique locale reste accessible.');
    updateAccount();
  }
}

async function onAuth() {
  apple.authorized = !!apple.music?.isAuthorized;
  document.body.classList.toggle('signed-out', !apple.authorized);
  if (apple.authorized) {
    try { const r = await api('/v1/me/storefront'); apple.sf = r.data?.[0]?.id || apple.sf; } catch { }
    loadPlaylists();
    session.restoreApple();
  } else {
    apple.sf = (navigator.language.split('-')[1] || 'fr').toLowerCase();
  }
  updateAccount();
  if (['home', 'recent', 'artists', 'albums', 'songs'].includes(router.view)) router.render();
}

function updateAccount() {
  const b = $('#account');
  $('#account-label').textContent = trad(!apple.ok ? 'Apple Music indisponible' : apple.authorized ? 'Apple Music' : 'Se connecter');
  b.classList.toggle('on', apple.authorized);
  b.title = trad(apple.authorized ? 'Connecté à Apple Music — cliquer pour se déconnecter' : 'Se connecter à Apple Music');
}

async function signIn(e) {
  if (!apple.ok) { await initApple(); if (!apple.ok) return; }
  if (apple.authorized) {
    // confirmation dans un menu de Pomme (pas de boîte de dialogue du moteur web)
    const r = e?.currentTarget?.getBoundingClientRect?.();
    const at = r ? { clientX: r.left, clientY: r.top - 50 } : { clientX: 20, clientY: innerHeight / 2 };
    menu.show(at, [['Se déconnecter d\'Apple Music', async () => { await apple.music.unauthorize(); onAuth(); }], ['Annuler', () => { }]]);
    return;
  }
  try { await apple.music.authorize(); } catch (e) { if (!isAbort(e)) toast('Connexion annulée'); }
  onAuth();
}

/** Appel à l'API Apple Music (même service que le lecteur web). */
async function api(path, { method = 'GET', body, cache = method === 'GET' } = {}) {
  // réponses gardées 3 min pour la bibliothèque (elle change), 30 min pour le catalogue
  const ttl = path.startsWith('/v1/me/') ? 180e3 : 1800e3;
  const hit = cache && apple.cache.get(path);
  if (hit && Date.now() - hit.t < ttl) return hit.j;
  if (!apple.token) throw new Error('Apple Music indisponible');
  const headers = { Authorization: 'Bearer ' + apple.token };
  if (apple.music?.musicUserToken) headers['Music-User-Token'] = apple.music.musicUserToken;
  if (body) headers['Content-Type'] = 'application/json';
  const sep = path.includes('?') ? '&' : '?';
  const url = 'https://amp-api.music.apple.com' + path + (!/[?&]l=/.test(path) ? sep + 'l=' + encodeURIComponent(LOCALE || navigator.language || 'fr-FR') : '');
  // compte : jamais le cache HTTP du moteur web (il gardait la 404 d'une playlist
  // vide, et les morceaux ajoutés ensuite restaient invisibles) ; Pomme a le sien
  const fetchCache = path.startsWith('/v1/me/') ? 'no-store' : 'default';
  let r = await fetch(url, { method, headers, body: body && JSON.stringify(body), cache: fetchCache });
  if (r.status === 401 && path.indexOf('/me/') < 0) {
    apple.token = await developerToken(true);
    headers.Authorization = 'Bearer ' + apple.token;
    r = await fetch(url, { method, headers, body: body && JSON.stringify(body), cache: fetchCache });
  }
  if (!r.ok) throw new Error('HTTP ' + r.status);
  const j = r.status === 204 ? {} : await r.json().catch(() => ({}));
  if (cache) { apple.cache.delete(path); apple.cache.set(path, { t: Date.now(), j }); if (apple.cache.size > 150) apple.cache.delete(apple.cache.keys().next().value); }
  return j;
}
const uncache = prefix => { for (const k of [...apple.cache.keys()]) if (k.startsWith(prefix)) apple.cache.delete(k); };

/** Toutes les pages d'une collection, livrées au fur et à mesure. */
async function apiAll(path, onPage, max = 5000) {
  const all = [];
  // le lien « page suivante » d'Apple perd les options (include, limit…) : on les y remet
  const keep = new URLSearchParams(path.split('?')[1] || '');
  const first = await api(path);
  for (const d of first.data || []) all.push(d);
  if (onPage(all) === false || !first.next) return all;
  const per = (first.data || []).length, total = Math.min(first.meta?.total || 0, max);
  if (per && total > all.length) {
    // total annoncé : pages restantes demandées 4 à la fois, livrées dans l'ordre
    const base = path.split('?')[0], offsets = [];
    for (let o = per; o < total; o += per) offsets.push(o);
    const page = async o => {
      const q = new URLSearchParams(keep); q.set('offset', o);
      try { return await api(base + '?' + q); } catch { return api(base + '?' + q); }
    };
    for (let i = 0; i < offsets.length; i += 4) {
      const got = await Promise.all(offsets.slice(i, i + 4).map(page));
      for (const r of got) for (const d of r.data || []) all.push(d);
      if (onPage(all) === false) break;
    }
    return all;
  }
  // sans total annoncé : pages suivies une à une
  const nextOf = link => {
    const [p, q = ''] = link.split('?'), qs = new URLSearchParams(q);
    for (const [k, v] of keep) if (!qs.has(k)) qs.set(k, v);
    return p + '?' + qs;
  };
  let next = nextOf(first.next);
  while (next && all.length < max) {
    const r = await api(next);
    for (const d of r.data || []) all.push(d);
    if (onPage(all) === false || !r.next) break;
    next = nextOf(r.next);
  }
  return all;
}

// ── bibliothèque locale et réglages natifs ───────────────────────────────
const local = { tracks: [], folders: [], byId: new Map(), scanning: false };
const app = { closeToTray: true, autostart: false, gpu: true, discord: true, discordId: '', version: '', devices: [] };

// ── lecteur unifié (Apple Music ou fichiers locaux) ─────────────────────
const player = {
  src: null,                      // 'apple' | 'local'
  volume: store.get('vol', 0.8),
  shuffle: false,                 // désactivé à chaque lancement : seul le bouton l'active
  repeat: store.get('repeat', 0),  // 0 non, 1 une, 2 toutes
  playing: false,
  queue: [], order: [], index: -1,
  pos: 0, posAt: 0, dur: 0, lastMedia: 0, seq: 0, lastTrack: null,

  current() {
    if (this.src === 'apple') {
      const it = apple.music?.nowPlayingItem;
      if (!it) return null;
      const a = it.attributes || {};
      return { id: it.id, title: a.name || it.title, artist: a.artistName || it.artistName, album: a.albumName || it.albumName, art: a.artwork || it.artwork || it.artworkURL, duration: (a.durationInMillis || 0) / 1000 || apple.music.currentPlaybackDuration, kind: 'apple', catalogId: a.playParams?.catalogId || it.id };
    }
    if (this.src === 'local') {
      const t = local.byId.get(this.queue[this.order[this.index]]);
      if (!t) return null;
      return { id: t.id, title: t.title, artist: t.artist, album: t.album, art: t.art ? 'art/' + t.id : '', duration: this.dur || t.duration, kind: 'local', track: t };
    }
    return null;
  },
  position() {
    if (this.src === 'apple') return apple.music?.currentPlaybackTime || 0;
    if (this.src === 'local') return this.pos + (this.playing ? (performance.now() - this.posAt) / 1000 : 0);
    return 0;
  },
  stopOther(src) {
    if (src !== 'apple' && this.src === 'apple' && apple.music) apple.music.stop()?.catch?.(() => { });
    if (src !== 'local' && this.src === 'local') native('audio.stop');
  },

  /** Seul le dernier clic compte : une demande remplacée par une plus
   *  récente n'affiche jamais d'erreur. */
  async playApple(desc) {
    if (!apple.ok) return toast('Apple Music est indisponible');
    if (!apple.authorized) toast('Connectez-vous pour écouter les morceaux en entier');
    const seq = ++this.seq;
    this.stopOther('apple');
    this.src = 'apple';
    for (let attempt = 0; ; attempt++) {
      try {
        // En aléatoire, MusicKit mélange la file avant de démarrer et ignore
        // startWith : on ne mélange qu'une fois le morceau choisi lancé
        // (il reste alors en tête). Sans morceau choisi (bouton Aléatoire),
        // le mélange d'avant garde un départ au hasard.
        apple.music.shuffleMode = this.shuffle && desc.startWith == null ? 1 : 0;
        await apple.music.setQueue({ ...desc, startPlaying: true });
        if (seq !== this.seq) return;
        apple.music.shuffleMode = this.shuffle ? 1 : 0;
        apple.music.repeatMode = this.repeat;
        return seq;
      } catch (e) {
        if (seq !== this.seq || isAbort(e)) return;
        const msg = String(e?.message || e);
        log('playApple: ' + msg + ' ' + JSON.stringify(desc).slice(0, 400));
        // Un morceau indisponible (retiré du catalogue, autre pays, import
        // iCloud non résolu) fait échouer toute la file : on le retire et
        // on relance, au pire avec le seul morceau choisi.
        const next = attempt < 2 && unresolved(desc, msg, attempt);
        if (next) { desc = next; continue; }
        return toast(/resolv|not found|unavailable/i.test(msg) ? trad("Ce morceau n'est pas disponible sur Apple Music") : trad('Lecture impossible : {e}', { e: msg }));
      }
    }
  },
  playLocal(ids, start = 0) {
    if (!ids.length) return;
    this.seq++;
    this.stopOther('local');
    this.src = 'local';
    this.queue = ids.slice();
    this.order = ids.map((_, i) => i);
    if (this.shuffle) { this.shuffleOrder(start); this.index = 0; } else this.index = start;
    this.loadLocal(true);
  },
  shuffleOrder(first) {
    const o = this.order.filter(i => i !== first);
    for (let i = o.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); [o[i], o[j]] = [o[j], o[i]]; }
    this.order = [first, ...o];
  },
  loadLocal(play, start = 0) {
    const id = this.queue[this.order[this.index]];
    this.pos = start; this.posAt = performance.now(); this.dur = local.byId.get(id)?.duration || 0;
    native('audio.load', { id, play, start, normalize: store.get('normalize', false) });
    this.onTrack();
  },
  async toggle() {
    try {
      if (this.src === 'apple') { apple.music.isPlaying ? await apple.music.pause() : await apple.music.play(); }
      else if (this.src === 'local') native(this.playing ? 'audio.pause' : 'audio.play');
    } catch (e) { if (!isAbort(e)) toast(tradErr(e.message || String(e))); }
  },
  pause() { if (this.playing) this.toggle(); },
  async next() {
    try {
      if (this.src === 'apple') return await apple.music.skipToNextItem();
      if (this.src !== 'local') return;
      if (this.index < this.order.length - 1) { this.index++; this.loadLocal(true); }
      else if (this.repeat === 2) { this.index = 0; this.loadLocal(true); }
    } catch (e) { if (!isAbort(e)) log(e); }
  },
  async prev() {
    try {
      if (this.position() > 4) return this.seek(0);
      if (this.src === 'apple') return await apple.music.skipToPreviousItem();
      if (this.src === 'local' && this.index > 0) { this.index--; this.loadLocal(true); }
      else if (this.src === 'local') this.seek(0);
    } catch (e) { if (!isAbort(e)) log(e); }
  },
  seek(t) {
    t = Math.max(0, t);
    if (this.src === 'apple') apple.music.seekToTime(t)?.catch?.(() => { });
    else if (this.src === 'local') { native('audio.seek', { position: t }); this.pos = t; this.posAt = performance.now(); }
    ui.progress(t, true);
    lyrics.syncNow();
  },
  setVolume(v) {
    this.volume = Math.max(0, Math.min(1, v)); store.set('vol', this.volume);
    if (apple.music) apple.music.volume = this.volume;
    native('audio.volume', { volume: this.volume });
    const vol = $('#vol'); vol.value = this.volume; vol.style.setProperty('--v', this.volume * 100 + '%');
  },
  setShuffle(on) {
    this.shuffle = on;
    if (this.src === 'apple' && apple.music) apple.music.shuffleMode = on ? 1 : 0;
    if (this.src === 'local' && this.order.length) {
      const cur = this.order[this.index];
      if (on) this.shuffleOrder(cur); else this.order = this.queue.map((_, i) => i);
      this.index = this.order.indexOf(cur);
    }
    ui.modes();
  },
  setRepeat(r) {
    this.repeat = r; store.set('repeat', r);
    if (apple.music) apple.music.repeatMode = r;
    ui.modes();
  },
  ended() {
    if (sleep.endOfTrack) { sleep.fire(); return; }
    if (this.repeat === 1) { this.loadLocal(true); return; }
    // fin de la file sans répétition : en pause au début du dernier morceau
    if (this.index >= this.order.length - 1 && this.repeat !== 2) {
      this.playing = false; this.loadLocal(false); this.onState();
      return;
    }
    this.next();
  },

  onState() {
    this.playing = this.src === 'apple' ? !!apple.music?.isPlaying : this.playing;
    ui.playState();
    this.media(true);
    lyrics.tick();
  },
  onTrack() {
    const id = this.current()?.id;
    if (sleep.endOfTrack && this.lastTrack && id && this.lastTrack !== id && this.src === 'apple') sleep.fire();
    this.lastTrack = id;
    ui.nowPlaying();
    lyrics.load();
    queueChanged();
    router.markPlaying();
    this.media(true);
  },
  onTime() {
    ui.progress(this.position());
    this.media(false);
  },
  /** Contrôles multimédia de Windows (toutes les 5 s, ou à chaque changement). */
  media(force) {
    const now = performance.now();
    if (!force && now - this.lastMedia < 5000) return;
    this.lastMedia = now;
    const c = this.current();
    if (!c) return;
    native('media', { title: c.title, artist: c.artist, album: c.album, art: c.kind === 'apple' ? artUrl(c.art, 300) : '', duration: c.duration, playing: this.playing, position: this.position() });
    session.save();
  },
};

// ── sortie audio : haut-parleurs, casque, enceintes Bluetooth… ──────────
// Les fichiers locaux passent par le moteur natif ; le lecteur d'Apple par
// les éléments audio de la page (setSinkId). Les deux suivent le même choix.
const output = {
  name: store.get('device', ''),
  sink: '',
  /** Sorties vues par le moteur web ; leurs noms ne sont lisibles qu'après
   *  une autorisation micro unique (aucun son capté, flux arrêté aussitôt). */
  async web(ask) {
    const outs = async () => (await navigator.mediaDevices.enumerateDevices()).filter(d => d.kind === 'audiooutput');
    let l = await outs();
    if (ask && !l.some(d => d.label)) {
      native('audio.labels');
      await new Promise(r => setTimeout(r, 120));
      try { (await navigator.mediaDevices.getUserMedia({ audio: true })).getTracks().forEach(t => t.stop()); } catch { }
      l = await outs();
    }
    return l;
  },
  async apply(ask) {
    let sink = '';
    if (this.name) {
      // le moteur web ajoute l'identifiant USB au nom (« … (046d:0af7) ») et des entrées « Par défaut » / « Communications »
      const same = d => !['default', 'communications'].includes(d.deviceId) && (d.label === this.name || d.label.startsWith(this.name + ' ('));
      try { sink = (await this.web(ask)).find(same)?.deviceId || ''; } catch { }
      if (!sink && ask && player.src === 'apple') toast('Apple Music reste sur la sortie de Windows (sortie introuvable)');
    }
    this.sink = sink;
    for (const m of document.querySelectorAll('audio, video')) if (m.setSinkId && m.sinkId !== sink) m.setSinkId(sink).catch(() => { });
  },
  async set(name) {
    this.name = name; store.set('device', name);
    native('audio.device', { name });
    await this.apply(true);
    toast(name ? trad('Sortie audio : {nom}', { nom: name }) : 'Sortie par défaut de Windows');
    if (router.view === 'settings') router.render();
  },
  /** Menu de la barre de lecture : liste rafraîchie à chaque ouverture. */
  menu(e) {
    const pos = { clientX: e.clientX, clientY: e.clientY };
    this.pendingMenu = pos;
    native('audio.devices');
  },
  showMenu() {
    const pos = this.pendingMenu; this.pendingMenu = null;
    if (!pos) return;
    const mark = n => (n === this.name ? '✓ ' : '   ');
    menu.show(pos, [[mark('') + trad('Sortie par défaut de Windows'), () => this.set('')], ...app.devices.map(d => [mark(d) + d, () => this.set(d)])]);
  },
};
// toute lecture du lecteur d'Apple part vers la sortie choisie
{
  const play = HTMLMediaElement.prototype.play;
  HTMLMediaElement.prototype.play = function () {
    if (this.setSinkId && this.sinkId !== output.sink) this.setSinkId(output.sink).catch(() => { });
    return play.call(this);
  };
}

// ── minuterie de mise en veille ─────────────────────────────────────────
const sleep = {
  timer: 0, endOfTrack: false, until: 0,
  set(min) {
    clearTimeout(this.timer); this.endOfTrack = false; this.until = 0;
    if (min === 'end') { this.endOfTrack = true; toast("La lecture s'arrêtera à la fin du morceau"); }
    else if (min > 0) { this.until = Date.now() + min * 60000; this.timer = setTimeout(() => this.fire(), min * 60000); toast(trad("La lecture s'arrêtera dans {n} min", { n: min })); }
  },
  fire() { this.set(0); player.pause(); toast('Minuterie : lecture arrêtée'); },
};

// ── reprise de la dernière écoute ───────────────────────────────────────
const session = {
  restored: false,
  save() {
    const c = player.current();
    if (!c) return;
    if (player.src === 'local') store.set('session', { src: 'local', queue: player.queue.slice(0, 3000), order: player.order.slice(0, 3000), index: player.index, pos: player.position() });
    else store.set('session', { src: 'apple', song: c.catalogId || c.id, pos: player.position() });
  },
  restoreLocal() {
    const s = store.get('session', null);
    if (this.restored || player.src || !s || s.src !== 'local' || !s.queue?.length) return;
    if (!s.queue.every(id => local.byId.has(id))) return;
    this.restored = true;
    player.src = 'local'; player.queue = s.queue;
    // l'aléatoire est coupé au lancement : la file reprend son ordre, sur le même morceau
    const order = s.order?.length === s.queue.length ? s.order : s.queue.map((_, i) => i);
    player.order = s.queue.map((_, i) => i);
    player.index = order[Math.max(0, Math.min(s.index || 0, order.length - 1))] ?? 0;
    player.loadLocal(false, s.pos || 0);
  },
  async restoreApple() {
    const s = store.get('session', null);
    if (this.restored || player.src || !s || s.src !== 'apple' || !s.song) return;
    this.restored = true;
    try {
      player.src = 'apple';
      await apple.music.setQueue({ song: s.song, startPlaying: false });
      if (!player.current()) { player.src = null; ui.progress(0, true); return; }
      player.onTrack();
      if (s.pos > 5) { apple.music.seekToTime(s.pos)?.catch?.(() => { }); ui.progress(s.pos, true); }
    } catch { if (!player.current()) { player.src = null; ui.progress(0, true); } }
  },
};

// ── barre de lecture, vue plein écran, mini-lecteur ──────────────────────
const ui = {
  duration: 0, lastPos: '',
  progress(t, jump) {
    const d = this.duration || 0;
    if (d) t = Math.min(t, d);
    const ratio = d ? Math.min(1, t / d) : 0;
    for (const fill of $$('.bar-fill')) {
      if (jump) fill.style.transition = 'none';
      fill.style.transform = `scaleX(${ratio})`;
    }
    if (jump) requestAnimationFrame(() => $$('.bar-fill').forEach(f => (f.style.transition = '')));
    $('#player').style.setProperty('--pp', (ratio * 100).toFixed(2) + '%');
    const s = fmtTime(t);
    if (this.lastPos !== s) { this.lastPos = s; for (const e of $$('.t-pos')) e.textContent = s; }
  },
  nowPlaying() {
    const c = player.current();
    const title = c ? c.title : 'Pomme';
    const sub = c ? [c.artist, c.album].filter(Boolean).join(' — ') : trad('Choisissez de la musique');
    $('#np-title').textContent = title; $('#np-sub').textContent = sub;
    $('#np-big-title').textContent = title; $('#np-big-sub').textContent = sub;
    $('#mini-title').textContent = title; $('#mini-sub').textContent = c?.artist || '';
    const small = c ? (c.kind === 'apple' ? artUrl(c.art, 56) : c.art) : '';
    const big = c ? (c.kind === 'apple' ? artUrl(c.art, 560) : c.art) : '';
    const tiny = c ? (c.kind === 'apple' ? artUrl(c.art, 32) : c.art) : '';
    setImg($('#np-art'), small);
    for (const [sel, src] of [['#np-big', big], ['#mini-art', big], ['#np-bg', tiny], ['#mini-bg', tiny]]) setImg($(sel), src, true);
    this.duration = c?.duration || 0;
    for (const e of $$('.t-dur')) e.textContent = fmtTime(this.duration);
    const badge = $('#np-badge');
    if (c?.kind === 'local') {
      const t = c.track;
      badge.hidden = false;
      badge.className = 'badge' + (t.lossless ? ' lossless' : '');
      badge.textContent = t.lossless ? `${t.format} ${t.bits || 16}-bit/${(t.rate / 1000).toFixed(t.rate % 1000 ? 1 : 0)} kHz` : `${t.format}${t.bitrate ? ' ' + t.bitrate + ' kb/s' : ''}`;
    } else if (c) { badge.hidden = false; badge.className = 'badge'; badge.textContent = 'AAC ' + bitrate(); }
    else badge.hidden = true;
    document.title = c ? `${c.title} — Pomme` : 'Pomme';
    ambient(tiny);
  },
  playState() {
    for (const i of $$('.play-ico')) i.innerHTML = player.playing ? ICON_PAUSE : ICON_PLAY;
    for (const b of $$('.play')) b.title = trad(player.playing ? 'Pause' : 'Lecture');
  },
  modes() {
    $('#shuffle').classList.toggle('on', player.shuffle);
    $('#repeat').classList.toggle('on', player.repeat > 0);
    $('.repeat-one').hidden = player.repeat !== 1;
    $('#np [data-act=shuffle]').classList.toggle('on', player.shuffle);
    $('#np [data-act=repeat]').classList.toggle('on', player.repeat > 0);
  },
};
function setImg(el, src, keep) {
  if (el.dataset.src === src) return;
  el.dataset.src = src;
  if (!src) { el.removeAttribute('src'); el.hidden = !keep; return; }
  if (!keep) el.hidden = true;
  el.onload = () => (el.hidden = false);
  el.onerror = () => { el.removeAttribute('src'); el.hidden = !keep; };
  el.src = src;
}

/** Couleurs d'ambiance tirées de la pochette (image réduite à 6×6). */
let ambientSrc = '';
function ambient(src) {
  if (!src || src === ambientSrc) return;
  ambientSrc = src;
  const im = new Image(); im.crossOrigin = 'anonymous';
  im.onload = () => {
    try {
      const c = document.createElement('canvas'); c.width = c.height = 6;
      const g = c.getContext('2d', { willReadFrequently: true }); g.drawImage(im, 0, 0, 6, 6);
      const d = g.getImageData(0, 0, 6, 6).data;
      const px = i => `rgba(${d[i]},${d[i + 1]},${d[i + 2]},.28)`;
      // sur le seul calque d'ambiance : pas de recalcul des styles de toute la page
      const amb = $('#ambient');
      amb.style.setProperty('--amb1', px(0));
      amb.style.setProperty('--amb2', px((5 * 6 + 5) * 4));
    } catch { }
  };
  im.src = src;
}

const fullscreen = {
  tab: store.get('npTab', 'lyrics'),
  open() {
    if (!player.current()) return toast("Lancez un morceau pour l'afficher en plein écran");
    $('#np').hidden = false;
    this.renderTab();
    if (panel.view === 'lyrics') panel.render();
    ui.progress(player.position(), true);
  },
  close() {
    $('#np').hidden = true;
    lyrics.render(panel.view === 'lyrics' ? $('#panel-body') : null);
  },
  /** Onglets Paroles / À suivre de la vue plein écran. */
  setTab(t) {
    this.tab = t; store.set('npTab', t);
    this.renderTab();
    if (panel.view === 'lyrics') panel.render();
  },
  renderTab() {
    const q = this.tab === 'queue';
    $$('#np .np-tabs button').forEach(b => b.classList.toggle('on', b.dataset.np === this.tab));
    $('#np-lyrics').hidden = q; $('#np-queue').hidden = !q;
    if (q) {
      // les paroles reviennent au panneau s'il les affiche, sinon leur animation s'arrête
      lyrics.render(panel.view === 'lyrics' ? $('#panel-body') : null);
      $('#np-queue').innerHTML = queueHtml();
      $('#np-queue .qitem.cur')?.scrollIntoView({ block: 'start' });
    } else {
      $('#np-queue').innerHTML = '';
      lyrics.render($('#np-lyrics'));
    }
  },
  get on() { return !$('#np').hidden; },
};
const mini = {
  on: false,
  set(on) {
    this.on = on;
    $('#mini').hidden = !on;
    native('window.mini', { on });
    if (on) ui.progress(player.position(), true);
  },
};

// ── panneau paroles / file d'attente ─────────────────────────────────────
const panel = {
  view: null,
  open(v) {
    if (this.view === v) return this.close();
    this.view = v;
    $('#panel').hidden = false;
    document.body.classList.add('with-panel');
    $$('#panel .tabs button').forEach(b => b.classList.toggle('on', b.dataset.panel === v));
    $('#lyrics-btn').classList.toggle('on', v === 'lyrics');
    $('#queue-btn').classList.toggle('on', v === 'queue');
    this.render();
  },
  close() {
    this.view = null;
    $('#panel').hidden = true;
    document.body.classList.remove('with-panel');
    $('#lyrics-btn').classList.remove('on'); $('#queue-btn').classList.remove('on');
    if (!fullscreen.on) lyrics.render(null);
  },
  render() {
    const body = $('#panel-body');
    if (this.view === 'lyrics') {
      if (fullscreen.on && fullscreen.tab === 'lyrics') body.innerHTML = `<div class="empty">${trad('Paroles affichées en plein écran')}</div>`;
      else lyrics.render(body);
      return;
    }
    if (this.view !== 'queue') return;
    if (!(fullscreen.on && fullscreen.tab === 'lyrics')) lyrics.render(null);
    body.innerHTML = queueHtml();
  },
};

/** File d'attente à partir du morceau en cours (300 au plus, pour rester léger). */
function queueHtml() {
  let items = [];
  if (player.src === 'apple' && apple.music) {
    const cur = apple.music.nowPlayingItemIndex ?? -1;
    const all = apple.music.queue?.items || [];
    for (let i = Math.max(0, cur); i < all.length && items.length < 300; i++) {
      const it = all[i];
      items.push({ i, cur: i === cur, title: it.attributes?.name || it.title, sub: it.attributes?.artistName || '', art: artUrl(it.attributes?.artwork, 40) });
    }
  } else if (player.src === 'local') {
    for (let i = Math.max(0, player.index); i < player.order.length && items.length < 300; i++) {
      const t = local.byId.get(player.queue[player.order[i]]);
      items.push({ i, cur: i === player.index, title: t?.title, sub: t?.artist, art: t?.art ? 'art/' + t.id : '' });
    }
  }
  if (!items.length) return `<div class="empty">${trad("La file d'attente est vide")}</div>`;
  return items.map(it => `<div class="qitem${it.cur ? ' cur' : ''}" data-i="${it.i}">${it.art ? img(it.art) : '<div class="ph"></div>'}<div class="c"><div>${esc(it.title)}</div><div class="muted">${esc(it.sub)}</div></div></div>`).join('');
}
/** Clic sur un morceau de la file : lecture à cet endroit. */
function queueClick(e) {
  const q = e.target.closest('.qitem');
  if (!q) return false;
  const i = +q.dataset.i;
  if (player.src === 'apple') apple.music.changeToMediaAtIndex(i)?.catch?.(() => { });
  else if (player.src === 'local') { player.index = i; player.loadLocal(true); }
  return true;
}
/** La file a changé : panneau et plein écran à jour s'ils l'affichent. */
function queueChanged() {
  if (panel.view === 'queue') panel.render();
  if (fullscreen.on && fullscreen.tab === 'queue') fullscreen.renderTab();
}

// ── paroles synchronisées ────────────────────────────────────────────────
/* Chaque ligne : { t, e, text, words?: [{t, e, text}], bg?: [...] }.
   La ligne active est blanche ; si les mots sont horodatés, chaque mot se
   remplit progressivement puis reste blanc jusqu'à la fin de la ligne. */
const lyrics = {
  key: '', data: null, source: '', loading: false, box: null, els: [], idx: -1, raf: 0, wordState: [],
  load() {
    const c = player.current();
    if (!c) return;
    const key = c.kind + ':' + c.id;
    if (key === this.key) return;
    this.key = key; this.data = null; this.loading = true;
    this.paint();
    if (c.kind === 'apple' && apple.authorized && c.catalogId && !String(c.catalogId).startsWith('i.')) this.fromApple(c, key).catch(() => this.fromNative(c, key));
    else this.fromNative(c, key);
  },
  async fromApple(c, key) {
    let ttml = null;
    for (const kind of ['syllable-lyrics', 'lyrics']) {
      try {
        const r = await api(`/v1/catalog/${apple.sf}/songs/${c.catalogId}/${kind}`);
        ttml = r.data?.[0]?.attributes?.ttml || r.data?.[0]?.attributes?.ttmlLocalizations;
        if (ttml) break;
      } catch { }
    }
    if (key !== this.key) return;
    if (!ttml) throw new Error('pas de paroles Apple');
    this.set(key, parseTTML(ttml), 'Apple Music');
  },
  fromNative(c, key) {
    // recherche en ligne (LRCLIB) désactivable : seuls les fichiers locaux sont alors lus
    if (!store.get('onlineLyrics', true) && c.kind !== 'local') { this.set(key, null); return; }
    native('lyrics', { key, title: c.title, artist: c.artist, album: c.album, duration: c.duration, localId: c.kind === 'local' ? c.id : '' });
  },
  receive(msg) {
    if (msg.key !== this.key) return;
    const l = msg.lyrics;
    if (l?.synced) this.set(msg.key, parseLRC(l.synced), l.source);
    else if (l?.plain) this.set(msg.key, { plain: true, lines: l.plain.split(/\r?\n/).map(text => ({ t: 0, text })) }, l.source);
    else this.set(msg.key, null);
  },
  set(key, data, source) {
    if (key !== this.key) return;
    this.loading = false; this.data = data; this.source = source;
    this.paint();
  },
  render(box) { this.box = box; this.paint(); },
  paint() {
    this.stop();
    const box = this.box;
    if (!box) return;
    this.els = []; this.idx = -1; this.wordState = [];
    if (!player.current()) { box.innerHTML = `<div class="empty">${trad('Lancez un morceau pour voir ses paroles')}</div>`; return; }
    if (this.loading) { box.innerHTML = '<div class="spinner"></div>'; return; }
    if (!this.data?.lines?.length) { box.innerHTML = `<div class="empty">${trad('Paroles indisponibles pour ce morceau')}</div>`; return; }
    const d = this.data;
    const words = ws => ws.map(w => `<span class="w">${esc(w.text)}</span>`).join('');
    box.innerHTML = `<div class="lyrics${d.plain ? ' plain' : ''}">` + d.lines.map((l, i) =>
      `<p class="l${l.words ? ' timed' : ''}${l.gap ? ' gap' : ''}" data-i="${i}">${l.words ? words(l.words) : esc(l.text || '♪')}${l.bg?.length ? `<span class="bgv">${words(l.bg)}</span>` : ''}</p>`).join('') +
      `<div class="src">${this.source ? esc(trad('Paroles : {source}', { source: this.source })) : ''}</div></div>`;
    this.els = $$('.lyrics .l', box);
    this.syncNow();
    this.tick();
  },
  /** Boucle d'affichage : seulement si les paroles sont visibles et que la musique joue. */
  tick() {
    if (this.raf || !this.box || !this.data || this.data.plain || !player.playing || document.hidden) return;
    const step = () => {
      this.raf = 0;
      if (!this.box || !player.playing || document.hidden || !this.data) return;
      this.sync(player.position() + 0.12);
      this.raf = requestAnimationFrame(step);
    };
    this.raf = requestAnimationFrame(step);
  },
  stop() { if (this.raf) cancelAnimationFrame(this.raf); this.raf = 0; },
  /** Nouvelle taille de fenêtre : la ligne en cours revient à sa place. */
  recenter() {
    const p = this.els?.[this.idx], box = this.box;
    if (p && box) box.scrollTo({ top: p.offsetTop - box.clientHeight * 0.36, behavior: 'auto' });
  },
  syncNow() { if (this.box && this.data && !this.data.plain && this.els.length) this.sync(player.position() + 0.12, true); },
  sync(t, jump) {
    const L = this.data.lines;
    let lo = 0, hi = L.length - 1, i = -1;
    while (lo <= hi) { const m = (lo + hi) >> 1; if (L[m].t <= t) { i = m; lo = m + 1; } else hi = m - 1; }
    if (i !== this.idx) {
      const old = this.els[this.idx];
      if (old) { old.classList.remove('on'); old.classList.add('past'); this.resetWords(this.idx); }
      if (jump) this.els.forEach((p, k) => p.classList.toggle('past', k < i));
      this.idx = i;
      const p = this.els[i];
      if (p) {
        p.classList.remove('past'); p.classList.add('on');
        const box = this.box;
        box.scrollTo({ top: p.offsetTop - box.clientHeight * 0.36, behavior: jump ? 'auto' : 'smooth' });
      }
    }
    const line = L[i], p = this.els[i];
    if (!line || !p || !line.words) return;
    // remplissage mot à mot (mots principaux puis voix de fond)
    const spans = p.getElementsByClassName('w');
    const all = line.bg ? line.words.concat(line.bg) : line.words;
    const st = this.wordState[i] || (this.wordState[i] = []);
    for (let k = 0; k < all.length && k < spans.length; k++) {
      const w = all[k], s = spans[k];
      const state = t >= w.e ? 2 : t >= w.t ? 1 : 0;
      if (state === 1) s.style.setProperty('--p', Math.round(((t - w.t) / Math.max(0.05, w.e - w.t)) * 100) + '%');
      if (st[k] !== state) {
        st[k] = state;
        s.classList.toggle('sung', state === 2);
        s.classList.toggle('cur', state === 1);
      }
    }
  },
  resetWords(i) {
    const p = this.els[i];
    if (!p) return;
    for (const s of p.getElementsByClassName('w')) { s.classList.remove('sung', 'cur'); s.style.removeProperty('--p'); }
    this.wordState[i] = [];
  },
};

function parseTime(s) {
  if (!s) return 0;
  if (/s$/.test(s)) return parseFloat(s);
  return s.split(':').map(parseFloat).reduce((a, b) => a * 60 + b, 0);
}
/** TTML d'Apple : mots horodatés (span « feuilles »), voix de fond à part. */
function parseTTML(xml) {
  const doc = new DOMParser().parseFromString(xml, 'application/xml');
  const lines = [];
  const leafWords = (root, bgOnly) => {
    const out = [];
    for (const s of root.getElementsByTagName('span')) {
      if (s.getElementsByTagName('span').length || !s.getAttribute('begin')) continue;
      let inBg = false;
      for (let a = s.parentElement; a && a !== root; a = a.parentElement) if (a.getAttribute('ttm:role') === 'x-bg') { inBg = true; break; }
      if (inBg !== bgOnly) continue;
      const after = s.nextSibling?.nodeType === 3 ? s.nextSibling.textContent : '';
      out.push({ t: parseTime(s.getAttribute('begin')), e: parseTime(s.getAttribute('end')), text: s.textContent + (after && /\s/.test(after) ? ' ' : '') });
    }
    return out;
  };
  for (const p of doc.getElementsByTagName('p')) {
    const t = parseTime(p.getAttribute('begin')), e = parseTime(p.getAttribute('end'));
    const words = leafWords(p, false), bg = leafWords(p, true);
    const text = words.length ? words.map(w => w.text).join('') : p.textContent;
    lines.push({ t, e, text, words: words.length ? words : null, bg: bg.length ? bg : null });
  }
  if (!lines.some(l => l.t > 0)) return { plain: true, lines };
  return { plain: false, lines: withGaps(lines) };
}
function parseLRC(text) {
  const lines = [];
  for (const raw of text.split(/\r?\n/)) {
    const tags = [...raw.matchAll(/\[(\d+):(\d+(?:[.:]\d+)?)\]/g)];
    if (!tags.length) continue;
    const body = raw.replace(/\[[^\]]*\]/g, '').replace(/<\d+:\d+(?:\.\d+)?>/g, '').trim();
    for (const m of tags) lines.push({ t: +m[1] * 60 + parseFloat(m[2].replace(':', '.')), text: body });
  }
  lines.sort((a, b) => a.t - b.t);
  for (let i = 0; i < lines.length; i++) lines[i].e = lines[i + 1]?.t ?? lines[i].t + 5;
  return { plain: false, lines: withGaps(lines.filter(l => l.text)) };
}
/** Passages instrumentaux (plus de 7 s sans paroles) : « • • • ». */
function withGaps(lines) {
  const out = [];
  if (lines[0]?.t > 7) out.push({ t: 0.5, e: lines[0].t, text: '• • •', gap: true });
  lines.forEach((l, i) => {
    out.push(l);
    const n = lines[i + 1];
    if (n && l.e && n.t - l.e > 7) out.push({ t: l.e + 0.3, e: n.t, text: '• • •', gap: true });
  });
  return out;
}

// ── listes virtuelles ────────────────────────────────────────────────────
function vlist(host, items, row, h = 52) {
  const scroller = $('#main');
  const pool = new Map();
  let raf = 0;
  host.classList.add('tracks');
  const size = () => (host.style.height = items.length * h + 'px');
  size();
  function draw() {
    raf = 0;
    if (!host.isConnected) return;
    const top = scroller.scrollTop - host.offsetTop, vh = scroller.clientHeight;
    const a = Math.max(0, Math.floor(top / h) - 8), b = Math.min(items.length, Math.ceil((top + vh) / h) + 8);
    for (const [i, n] of pool) if (i < a || i >= b) { n.remove(); pool.delete(i); }
    const frag = document.createDocumentFragment();
    for (let i = a; i < b; i++) if (!pool.has(i)) {
      const n = row(items[i], i); n.__item = items[i]; n.dataset.i = i; n.style.transform = `translateY(${i * h}px)`;
      frag.appendChild(n); pool.set(i, n);
    }
    host.appendChild(frag);
  }
  const onScroll = () => { if (!raf) raf = requestAnimationFrame(draw); };
  scroller.addEventListener('scroll', onScroll, { passive: true });
  addEventListener('resize', onScroll);
  router.cleanup.push(() => { scroller.removeEventListener('scroll', onScroll); removeEventListener('resize', onScroll); });
  draw();
  return {
    h,
    // seules les lignes dont le morceau a changé sont refaites (pas de clignotement
    // ni de pochettes rechargées quand une page de plus arrive)
    update(next) {
      for (const [i, n] of pool) if (next[i] !== n.__item) { n.remove(); pool.delete(i); }
      items = next; size(); draw();
    },
    get items() { return items; },
  };
}

function trackRow(t) {
  const n = document.createElement('div');
  n.className = 'trow';
  n.innerHTML = `<div class="grip" title="${trad('Glisser pour déplacer')}">⠿</div><div class="art">${img(t.art)}</div><div class="c t">${esc(t.title)}<small>${esc(t.artist)}</small></div><div class="c sub art-c">${esc(t.artist)}</div><div class="c sub alb">${esc(t.album)}</div><div class="fmt">${esc(t.fmt || '')}</div><div class="dur">${fmtTime(t.duration)}</div><button class="act" title="${trad('Retirer')}">✕</button>`;
  const c = player.current();
  if (c && (c.id === t.id || (t.catalogId && c.catalogId === t.catalogId))) n.classList.add('playing');
  return n;
}

const songOf = s => {
  const a = s.attributes || {};
  return { id: s.id, type: s.type, title: a.name, artist: a.artistName, album: a.albumName, duration: (a.durationInMillis || 0) / 1000, art: artUrl(a.artwork, 40), catalogId: a.playParams?.catalogId || (s.type === 'songs' ? s.id : ''), raw: s };
};
const localOf = t => ({ id: t.id, title: t.title, artist: t.artist, album: t.album, duration: t.duration, art: t.art ? 'art/' + t.id : '', fmt: t.format, local: true });

/** Liste de morceaux : en-tête, lignes virtuelles, lecture au double-clic.
 *  `opts.editable` : poignées de déplacement ; `opts.onReorder(items)`. */
function songTable(host, songs, onPlay, opts = {}) {
  host.insertAdjacentHTML('beforeend', `<div class="thead"><span></span><span></span><span>${trad('Titre')}</span><span class="art-c">${trad('Artiste')}</span><span class="alb">${trad('Album')}</span><span class="fmt">${trad('Format')}</span><span class="r">${trad('Durée')}</span><span></span></div>`);
  const head = host.lastElementChild;
  const box = document.createElement('div');
  host.appendChild(box);
  const list = vlist(box, songs, trackRow);
  if (opts.editable) box.classList.add('editable');
  box.addEventListener('dblclick', e => { const r = e.target.closest('.trow'); if (r && !e.target.closest('.grip,.act')) onPlay(list.items, +r.dataset.i); });
  box.addEventListener('click', e => {
    const r = e.target.closest('.trow');
    if (!r) return;
    if (e.target.closest('.act')) { opts.onRemove?.(+r.dataset.i); return; }
    if (e.target.closest('.art')) onPlay(list.items, +r.dataset.i);
  });
  box.addEventListener('contextmenu', e => { const r = e.target.closest('.trow'); if (r) { e.preventDefault(); menu.song(e, list.items[+r.dataset.i], { ...opts, index: +r.dataset.i }); } });
  if (opts.editable) dragReorder(box, list, opts);
  list.box = box; list.head = head;
  return list;
}

/** Glisser-déposer à la souris (poignée ⠿), avec défilement automatique. */
function dragReorder(box, list, opts) {
  box.addEventListener('pointerdown', e => {
    const grip = e.target.closest('.grip');
    if (!grip || e.button !== 0) return;
    const row = grip.closest('.trow');
    const from = +row.dataset.i, h = list.h;
    const scroller = $('#main');
    e.preventDefault();
    try { grip.setPointerCapture(e.pointerId); } catch { }
    row.classList.add('dragging');
    const line = document.createElement('div'); line.className = 'drop-line'; box.appendChild(line);
    let y = e.clientY, to = from, raf = 0;
    const place = () => {
      raf = 0;
      const r = box.getBoundingClientRect();
      const pos = Math.max(0, Math.min(list.items.length * h, y - r.top));
      to = Math.max(0, Math.min(list.items.length, Math.round(pos / h)));
      line.style.top = to * h + 'px';
      row.style.transform = `translateY(${Math.max(0, pos - h / 2)}px)`;
      // défilement automatique près des bords
      const sr = scroller.getBoundingClientRect();
      const auto = y < sr.top + 60 ? -14 : y > sr.bottom - 60 ? 14 : 0;
      if (auto) { scroller.scrollTop += auto; raf = requestAnimationFrame(place); }
    };
    const move = ev => { y = ev.clientY; if (!raf) raf = requestAnimationFrame(place); };
    const up = () => {
      grip.removeEventListener('pointermove', move); grip.removeEventListener('pointerup', up); grip.removeEventListener('pointercancel', up);
      cancelAnimationFrame(raf); line.remove(); row.classList.remove('dragging');
      const dest = to > from ? to - 1 : to;
      if (dest !== from) {
        const items = list.items.slice();
        const [it] = items.splice(from, 1); items.splice(dest, 0, it);
        list.update(items);
        opts.onReorder?.(items);
      } else row.style.transform = `translateY(${from * h}px)`;
    };
    grip.addEventListener('pointermove', move); grip.addEventListener('pointerup', up); grip.addEventListener('pointercancel', up);
    place();
  });
}

/** Identifiant à mettre en file : celui de la bibliothèque pour un morceau de
 *  la bibliothèque (toujours résolu, et bien plus vite), sinon celui du catalogue. */
const queueId = s => (s.type === 'library-songs' && s.id) || s.catalogId || s.id;
/** Petite file autour du morceau choisi pour démarrer vite (Apple résout
 *  chaque identifiant avant de jouer), la suite est ajoutée en arrière-plan. */
const playAppleSongs = async (items, i) => {
  const from = Math.max(0, i - 10), end = i + 100;
  const seq = await player.playApple({ songs: items.slice(from, end).map(queueId), startWith: i - from, startPosition: i - from });
  const rest = items.slice(end, end + 1500).map(queueId);
  if (seq && seq === player.seq && rest.length) apple.music.playLater({ songs: rest }).catch(e => log('playLater: ' + (e?.message || e)));
};
const playLocalSongs = (items, i) => player.playLocal(items.map(s => s.id), i);
/** Boutons « Aléatoire » des pages : une lecture mélangée, sans activer le
 *  mode aléatoire du lecteur pour les lectures suivantes. */
const shuffled = items => {
  const o = items.slice();
  for (let i = o.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); [o[i], o[j]] = [o[j], o[i]]; }
  return o;
};

// ── cartes (albums, playlists, artistes, stations) ───────────────────────
function card(item, size = 172) {
  const a = item.attributes || {};
  const type = item.type || '';
  const title = a.name || a.title?.stringForDisplay || '';
  const sub = a.artistName || a.curatorName || (type.includes('playlist') ? 'Playlist' : type.includes('station') ? 'Radio' : '');
  return `<div class="card${type.includes('artist') ? ' round' : ''}" data-type="${esc(type)}" data-id="${esc(item.id)}"><div class="art">${img(artUrl(a.artwork, size))}${type.includes('artist') ? '' : `<span class="hover-play"><svg viewBox="0 0 24 24">${ICON_PLAY}</svg></span>`}</div><div class="t">${esc(title)}</div><div class="s">${esc(sub)}</div></div>`;
}
/** Clics sur les cartes : un seul écouteur sur #view, posé au démarrage (un
 *  écouteur ajouté à chaque page s'accumulait et rejouait chaque clic N fois). */
function bindCards(root) {
  root.addEventListener('click', e => {
    const c = e.target.closest('.card');
    if (!c) return;
    const { type = '', id } = c.dataset;
    if (c.dataset.key !== undefined) return router.go('local-album', c.dataset.key);
    if (c.dataset.name !== undefined) return router.go('search', c.dataset.name);
    if (e.target.closest('.hover-play') || type.includes('station')) return playItem(type, id);
    if (type.includes('artist')) return router.go('artist', id);
    if (type.includes('playlist')) return router.go('playlist', id);
    if (type.includes('album')) return router.go('album', id);
  });
}
function playItem(type, id) {
  if (type.includes('station')) return player.playApple({ station: id });
  if (type.includes('playlist')) return player.playApple({ playlist: id });
  if (type.includes('album')) return player.playApple({ album: id });
  if (type.includes('song')) return player.playApple({ song: id });
}
function section(title, items, size) {
  if (!items?.length) return '';
  return `<div class="section"><h2>${esc(trad(title))}</h2><div class="shelf">${items.slice(0, 24).map(i => card(i, size)).join('')}</div></div>`;
}

// ── menu contextuel ──────────────────────────────────────────────────────
const menu = {
  el: null,
  close() { this.el?.remove(); this.el = null; },
  show(e, items) {
    this.close();
    const m = document.createElement('div');
    m.className = 'menu';
    m.innerHTML = items.map((it, i) => `<div class="qitem" data-i="${i}">${esc(trad(it[0]))}</div>`).join('');
    m.addEventListener('click', ev => { const r = ev.target.closest('[data-i]'); if (r) { items[+r.dataset.i][1](); this.close(); } });
    document.body.appendChild(m);
    m.style.left = Math.max(8, Math.min(e.clientX, innerWidth - m.offsetWidth - 8)) + 'px';
    m.style.top = Math.max(8, Math.min(e.clientY, innerHeight - m.offsetHeight - 8)) + 'px';
    this.el = m;
  },
  song(e, s, opts = {}) {
    if (s.local) {
      return this.show(e, [
        ['Lire ensuite', () => { if (player.src === 'local') { player.queue.push(s.id); player.order.splice(player.index + 1, 0, player.queue.length - 1); toast('Ajouté à la file'); } else player.playLocal([s.id]); }],
        ['Afficher l\'album', () => router.go('local-album', (local.byId.get(s.id)?.album_artist || '') + '\u0001' + (s.album || ''))],
      ]);
    }
    const id = s.catalogId || s.id;
    const items = [
      ['Lire ensuite', () => apple.music.playNext({ song: id }).then(() => toast('Lu ensuite'), err => !isAbort(err) && toast('Impossible'))],
      ['Lire plus tard', () => apple.music.playLater({ song: id }).then(() => toast('Ajouté à la file'), err => !isAbort(err) && toast('Impossible'))],
    ];
    if (apple.authorized) {
      if (!String(s.type).startsWith('library')) items.push(['Ajouter à la bibliothèque', () => api(`/v1/me/library?ids[songs]=${id}`, { method: 'POST', cache: false }).then(() => toast('Ajouté à la bibliothèque'), () => toast('Échec de l\'ajout'))]);
      items.push(['J\'aime', () => api(`/v1/me/ratings/songs/${id}`, { method: 'PUT', cache: false, body: { type: 'rating', attributes: { value: 1 } } }).then(() => toast('Ajouté à vos favoris'), () => toast('Échec'))]);
      if (opts.onRemove && opts.index >= 0) items.push(['Retirer de la playlist', () => opts.onRemove(opts.index)]);
      for (const p of apple.playlists.filter(p => p.attributes?.canEdit).slice(0, 15)) {
        items.push(['➕ ' + p.attributes.name, () => addToPlaylist(p.id, [id]).then(() => toast(trad('Ajouté à « {nom} »', { nom: p.attributes.name })), () => toast('Échec'))]);
      }
    }
    this.show(e, items);
  },
};
addEventListener('pointerdown', e => { if (menu.el && !menu.el.contains(e.target)) menu.close(); });
/** Saisie d'un texte dans Pomme (la boîte prompt() du moteur web bloque la page
 *  et s'affiche hors du thème). Résout le texte, ou null si annulé. */
function ask(title, value = '') {
  return new Promise(done => {
    menu.close();
    const m = document.createElement('div');
    m.className = 'menu ask';
    m.innerHTML = `<div class="ask-t">${esc(trad(title))}</div><input class="field" maxlength="100" value="${esc(value)}"><div class="ask-b"><button class="btn" data-a="0">${trad('Annuler')}</button><button class="btn primary" data-a="1">OK</button></div>`;
    document.body.appendChild(m);
    menu.el = m;
    const inp = $('input', m);
    // la promesse garde la première réponse : celle-ci passe avant le null de menu.close()
    const end = ok => { const v = inp.value.trim(); done(ok && v ? v : null); menu.close(); };
    m.addEventListener('click', e => { const b = e.target.closest('[data-a]'); if (b) end(b.dataset.a === '1'); });
    inp.addEventListener('keydown', e => { if (e.key === 'Enter') end(true); else if (e.key === 'Escape') end(false); });
    // fermeture par un clic à côté : annulé
    const close = menu.close.bind(menu);
    menu.close = () => { menu.close = close; close(); done(null); };
    inp.focus(); inp.select();
  });
}
/** Morceaux d'une playlist de la bibliothèque ; vide → [] (Apple répond 404). */
const playlistTracks = id => apiAll(`/v1/me/library/playlists/${id}/tracks?limit=100`, () => { }).catch(e => /HTTP 404/.test(e.message) ? [] : Promise.reject(e));
const addToPlaylist = (pid, songIds) => api(`/v1/me/library/playlists/${pid}/tracks`, { method: 'POST', cache: false, body: { data: songIds.map(id => ({ id, type: String(id).startsWith('i.') ? 'library-songs' : 'songs' })) } }).then(r => { uncache(`/v1/me/library/playlists/${pid}`); return r; });

// ── navigation ───────────────────────────────────────────────────────────
const router = {
  view: 'home', arg: null, history: [], cleanup: [], token: 0,
  go(view, arg, push = true) {
    if (push && (view !== this.view || arg !== this.arg)) this.history.push([this.view, this.arg]);
    this.view = view; this.arg = arg;
    if (!['album', 'playlist', 'artist', 'search', 'local-album'].includes(view)) store.set('view', view);
    $$('#nav a').forEach(a => a.classList.toggle('on', a.dataset.view === view || (view === 'playlist' && !!a.dataset.pl && a.dataset.pl === arg)));
    document.body.classList.remove('drawer');
    if (fullscreen.on) fullscreen.close();
    this.render();
  },
  back() { const h = this.history.pop(); if (h) this.go(h[0], h[1], false); },
  render() {
    this.cleanup.forEach(f => f()); this.cleanup = [];
    menu.close();
    const v = $('#view');
    $('#main').scrollTop = 0;
    const token = ++this.token;
    const live = () => token === this.token;
    v.innerHTML = '<div class="spinner"></div>';
    const fn = views[this.view] || views.home;
    const ready = this.view.startsWith('local') || this.view === 'settings' ? null : appleReady;
    Promise.resolve(ready).then(() => live() && fn(v, this.arg, live)).catch(e => {
      log('vue ' + this.view + ' en erreur : ' + (e.message || e));
      if (live()) v.innerHTML = `<div class="empty"><h2>${trad('Impossible de charger cette page')}</h2><div>${esc(e.message || e)}</div></div>`;
    });
  },
  markPlaying() {
    const c = player.current();
    for (const r of $$('.trow')) {
      const t = r.__item;
      r.classList.toggle('playing', !!c && !!t && (t.id === c.id || (!!t.catalogId && t.catalogId === c.catalogId)));
    }
  },
};

function needAuth(v) {
  v.innerHTML = `<div class="empty"><h2>${trad('Connectez-vous à Apple Music')}</h2><div>${trad('Votre bibliothèque, vos playlists et les recommandations apparaissent une fois connecté.')}</div><button class="btn primary" id="signin2">${trad('Se connecter')}</button></div>`;
  $('#signin2').onclick = signIn;
}

const views = {
  async home(v, _, live) {
    if (!apple.ok) return views['local-albums'](v, null, live, 'Votre musique');
    if (!apple.authorized) return views.browse(v, null, live, true);
    const [rec, recent] = await Promise.all([
      api(`/v1/me/recommendations?limit=10&types=albums,playlists,stations`).catch(() => ({ data: [] })),
      api(`/v1/me/recent/played?limit=20`).catch(() => ({ data: [] })),
    ]);
    if (!live()) return;
    let html = `<h1>${trad('Écouter maintenant')}</h1>` + section('Écoutés récemment', recent.data);
    const seen = new Set(['Écoutés récemment']);
    for (const r of rec.data || []) {
      const title = r.attributes?.title?.stringForDisplay || 'Pour vous';
      if (seen.has(title)) continue;
      seen.add(title);
      html += section(title, r.relationships?.contents?.data);
    }
    v.innerHTML = html || `<div class="empty">${trad("Rien à afficher pour l'instant")}</div>`;
  },
  async browse(v, _, live, invite) {
    const r = await api(`/v1/catalog/${apple.sf}/charts?types=albums,playlists,songs&limit=24`);
    if (!live()) return;
    const ch = r.results || {};
    let html = invite ? `<div class="card-box glass" style="display:flex;align-items:center;gap:16px;flex-wrap:wrap"><div style="flex:1 1 260px"><b>${trad('Écoutez Apple Music en entier')}</b><div class="muted">${trad('Connectez-vous avec votre identifiant Apple pour accéder à votre bibliothèque et lire les morceaux complets.')}</div></div><button class="btn primary" id="signin3">${trad('Se connecter')}</button></div>` : '';
    html += `<h1>${trad('Parcourir')}</h1>` + section('Albums du moment', ch.albums?.[0]?.data) + section('Playlists populaires', ch.playlists?.[0]?.data);
    v.innerHTML = html;
    if (ch.songs?.[0]?.data?.length) { v.insertAdjacentHTML('beforeend', `<h2>${trad('Top morceaux')}</h2>`); songTable(v, ch.songs[0].data.map(songOf), playAppleSongs); }
    $('#signin3')?.addEventListener('click', signIn);
  },
  async radio(v, _, live) {
    const [liveR, personal] = await Promise.all([
      api(`/v1/catalog/${apple.sf}/stations?filter[featured]=apple-music-live-radio`).catch(() => ({ data: [] })),
      apple.authorized ? api(`/v1/catalog/${apple.sf}/stations?filter[identity]=personal`).catch(() => ({ data: [] })) : { data: [] },
    ]);
    if (!live()) return;
    v.innerHTML = `<h1>${trad('Radio')}</h1>` + section('En direct', liveR.data, 220) + section('Votre station', personal.data, 220);
    if (!v.querySelector('.card')) v.insertAdjacentHTML('beforeend', `<div class="empty">${trad('Aucune radio disponible')}</div>`);
  },
  async search(v, q, live) {
    q = (q || '').trim();
    if (!q) { v.innerHTML = `<div class="empty">${trad('Tapez un titre, un artiste ou un album')}</div>`; return; }
    const ql = q.toLowerCase();
    const localHits = local.tracks.filter(t => (t.title + ' ' + t.artist + ' ' + t.album).toLowerCase().includes(ql)).slice(0, 300);
    let r = { results: {} }, lib = { results: {} };
    if (apple.ok) [r, lib] = await Promise.all([
      api(`/v1/catalog/${apple.sf}/search?term=${encodeURIComponent(q)}&types=songs,albums,artists,playlists,stations&limit=20`).catch(() => ({ results: {} })),
      apple.authorized ? api(`/v1/me/library/search?term=${encodeURIComponent(q)}&types=library-songs,library-playlists&limit=25`).catch(() => ({ results: {} })) : { results: {} },
    ]);
    if (!live()) return;
    const R = r.results || {}, L = lib.results || {};
    v.innerHTML = `<h1>« ${esc(q)} »</h1>` + section('Artistes', R.artists?.data, 150) + section('Albums', R.albums?.data) + section('Playlists', R.playlists?.data) + section('Vos playlists', L['library-playlists']?.data) + section('Radios', R.stations?.data);
    if (R.songs?.data?.length) { v.insertAdjacentHTML('beforeend', '<h2>Morceaux</h2>'); songTable(v, R.songs.data.map(songOf), playAppleSongs); }
    if (L['library-songs']?.data?.length) { v.insertAdjacentHTML('beforeend', `<h2>${trad('Dans votre bibliothèque')}</h2>`); songTable(v, L['library-songs'].data.map(songOf), playAppleSongs); }
    if (localHits.length) { v.insertAdjacentHTML('beforeend', `<h2>${trad('Sur cet ordinateur')}</h2>`); songTable(v, localHits.map(localOf), playLocalSongs); }
    if (!v.querySelector('.card, .tracks')) v.insertAdjacentHTML('beforeend', `<div class="empty">${trad('Aucun résultat')}</div>`);
  },
  async recent(v, _, live) {
    if (!apple.authorized) return needAuth(v);
    const r = await api('/v1/me/library/recently-added?limit=25');
    if (!live()) return;
    v.innerHTML = `<h1>${trad('Ajouts récents')}</h1><div class="grid">${(r.data || []).map(i => card(i)).join('')}</div>`;
  },
  async albums(v, _, live) {
    if (!apple.authorized) return needAuth(v);
    v.innerHTML = `<h1>${trad('Albums')}</h1><div class="grid" id="g"></div>`;
    const g = $('#g', v);
    let shown = 0;
    await apiAll('/v1/me/library/albums?limit=100', all => {
      if (!live()) return false;
      g.insertAdjacentHTML('beforeend', all.slice(shown).map(i => card(i)).join('')); shown = all.length;
    });
  },
  async artists(v, _, live) {
    if (!apple.authorized) return needAuth(v);
    v.innerHTML = `<h1>${trad('Artistes')}</h1><div class="grid" id="g"></div>`;
    const g = $('#g', v);
    let shown = 0;
    await apiAll('/v1/me/library/artists?limit=100&include=catalog', all => {
      if (!live()) return false;
      g.insertAdjacentHTML('beforeend', all.slice(shown).map(a => {
        const name = a.attributes?.name, cat = a.relationships?.catalog?.data?.[0];
        const to = cat?.id ? `data-type="artists" data-id="${esc(cat.id)}"` : `data-name="${esc(name)}"`;
        return `<div class="card round" ${to}>${artistArt(name, cat?.attributes?.artwork)}<div class="t">${esc(name)}</div></div>`;
      }).join('')); shown = all.length;
    });
  },
  async songs(v, _, live) {
    if (!apple.authorized) return needAuth(v);
    v.innerHTML = `<div class="row-head"><h1>${trad('Morceaux')}</h1><span class="muted" id="cnt"></span><button class="btn" id="sh">${trad('Aléatoire')}</button></div>`;
    let list = null;
    $('#sh').onclick = () => { if (list?.items.length) playAppleSongs(shuffled(list.items), 0); };
    const s = [];
    await apiAll('/v1/me/library/songs?limit=100', all => {
      if (!live()) return false;
      // seules les nouvelles pages sont converties
      for (let k = s.length; k < all.length; k++) s.push(songOf(all[k]));
      if (!list) list = songTable(v, s, playAppleSongs); else list.update(s);
      $('#cnt').textContent = tradn(all.length, '{n} morceau', '{n} morceaux');
    }, 20000);
  },
  async album(v, id, live) {
    const lib = String(id).startsWith('l.');
    const r = await api(lib ? `/v1/me/library/albums/${id}?include=tracks` : `/v1/catalog/${apple.sf}/albums/${id}?include=tracks`);
    if (!live()) return;
    const al = r.data?.[0]; const a = al?.attributes || {};
    const tracks = al?.relationships?.tracks?.data || [];
    v.innerHTML = `<div class="hero"><div class="cover">${img(artUrl(a.artwork, 230))}</div><div class="info"><div class="muted">${trad('Album')}</div><h1>${esc(a.name)}</h1><div>${esc(a.artistName)}</div><div class="muted">${esc([a.genreNames?.[0], (a.releaseDate || '').slice(0, 4), tradn(tracks.length, '{n} morceau', '{n} morceaux')].filter(Boolean).join(' · '))}</div><div class="actions"><button class="btn primary" id="pa">▶ ${trad('Lecture')}</button><button class="btn" id="sh">${trad('Aléatoire')}</button>${apple.authorized && !lib ? `<button class="btn" id="add">＋ ${trad('Bibliothèque')}</button>` : ''}</div></div></div>`;
    songTable(v, tracks.map(songOf), (items, i) => player.playApple({ album: id, startWith: i, startPosition: i }));
    $('#pa').onclick = () => player.playApple({ album: id });
    $('#sh').onclick = () => tracks.length ? playAppleSongs(shuffled(tracks.map(songOf)), 0) : player.playApple({ album: id });
    $('#add')?.addEventListener('click', () => api(`/v1/me/library?ids[albums]=${id}`, { method: 'POST', cache: false }).then(() => toast('Album ajouté à la bibliothèque'), () => toast('Échec de l\'ajout')));
  },
  async playlist(v, id, live) { return playlistView(v, id, live); },
  async artist(v, id, live) {
    const r = await api(`/v1/catalog/${apple.sf}/artists/${id}?views=top-songs,full-albums,singles,similar-artists,latest-release`);
    if (!live()) return;
    const ar = r.data?.[0]; const a = ar?.attributes || {}; const vw = ar?.views || {};
    v.innerHTML = `<div class="hero"><div class="cover round">${img(artUrl(a.artwork, 230))}</div><div class="info"><div class="muted">${trad('Artiste')}</div><h1>${esc(a.name)}</h1><div class="muted">${esc(a.genreNames?.join(', ') || '')}</div><div class="actions"><button class="btn primary" id="pa">▶ ${trad('Top morceaux')}</button></div></div></div>`;
    const top = (vw['top-songs']?.data || []).map(songOf);
    $('#pa').onclick = () => top.length && playAppleSongs(top, 0);
    if (top.length) { v.insertAdjacentHTML('beforeend', `<h2>${trad('Top morceaux')}</h2>`); songTable(v, top.slice(0, 10), playAppleSongs); }
    v.insertAdjacentHTML('beforeend', section('Dernière sortie', vw['latest-release']?.data) + section('Albums', vw['full-albums']?.data) + section('Singles et EP', vw.singles?.data) + section('Artistes similaires', vw['similar-artists']?.data, 150));
  },

  // musique locale
  'local-songs'(v) {
    if (!local.tracks.length) return localEmpty(v);
    v.innerHTML = `<div class="row-head"><h1>${trad('Morceaux')}</h1><span class="muted">${tradn(local.tracks.length, '{n} morceau', '{n} morceaux')}</span><button class="btn" id="sh">${trad('Aléatoire')}</button></div>`;
    const items = local.tracks.map(localOf);
    songTable(v, items, playLocalSongs);
    $('#sh').onclick = () => playLocalSongs(shuffled(items), 0);
  },
  'local-albums'(v, _, live, title) {
    if (!local.tracks.length) return localEmpty(v);
    const albums = new Map();
    for (const t of local.tracks) { const k = (t.album_artist || '') + '\u0001' + (t.album || ''); if (!albums.has(k)) albums.set(k, t); }
    v.innerHTML = `<h1>${esc(trad(title || 'Albums'))}</h1><div class="grid">${[...albums].map(([k, t]) => `<div class="card" data-key="${esc(k)}"><div class="art">${t.art ? img('art/' + t.id) : ''}</div><div class="t">${esc(t.album || trad('Sans album'))}</div><div class="s">${esc(t.album_artist || t.artist)}</div></div>`).join('')}</div>`;
  },
  'local-artists'(v) {
    if (!local.tracks.length) return localEmpty(v);
    const artists = [...new Set(local.tracks.map(t => t.album_artist || t.artist || trad('Artiste inconnu')))].sort((a, b) => a.localeCompare(b, LOCALE));
    const photos = store.get('artistPhotos', {});
    v.innerHTML = `<h1>${trad('Artistes')}</h1><div class="grid">${artists.map(a => `<div class="card round" data-name="${esc(a)}">${artistArt(a, photos[a])}<div class="t">${esc(a)}</div></div>`).join('')}</div>`;
    artistPhotos(v, photos);
  },
  'local-album'(v, key) {
    const [aa, al] = String(key).split('\u0001');
    const tracks = local.tracks.filter(t => (t.album_artist || '') === aa && (t.album || '') === al);
    const t0 = tracks[0];
    if (!t0) return router.go('local-albums');
    const items = tracks.map(localOf);
    v.innerHTML = `<div class="hero"><div class="cover">${t0.art ? img('art/' + t0.id) : ''}</div><div class="info"><div class="muted">${trad('Album · sur cet ordinateur')}</div><h1>${esc(al || trad('Sans album'))}</h1><div>${esc(aa || t0.artist)}</div><div class="muted">${esc([t0.genre, t0.year || '', tradn(tracks.length, '{n} morceau', '{n} morceaux'), t0.lossless ? 'Lossless' : t0.format].filter(Boolean).join(' · '))}</div><div class="actions"><button class="btn primary" id="pa">▶ ${trad('Lecture')}</button><button class="btn" id="sh">${trad('Aléatoire')}</button></div></div></div>`;
    songTable(v, items, playLocalSongs);
    $('#pa').onclick = () => playLocalSongs(items, 0);
    $('#sh').onclick = () => playLocalSongs(shuffled(items), 0);
  },
  settings(v) { settings.render(v); },
};

/** Photos des artistes locaux, cherchées dans le catalogue Apple à
 *  l'affichage de chaque carte (une requête à la fois, résultat mémorisé,
 *  '' si aucun artiste ne correspond). */
function artistPhotos(v, photos) {
  if (!apple.ok || !window.IntersectionObserver) return;
  const queue = []; let busy = false;
  const pump = async () => {
    if (busy) return; busy = true;
    while (queue.length && v.isConnected) {
      const card = queue.shift(), name = card.dataset.name;
      if (name in photos) continue;
      try {
        const r = await api(`/v1/catalog/${apple.sf}/search?types=artists&limit=1&term=${encodeURIComponent(name)}`);
        const ar = r.results?.artists?.data?.[0];
        photos[name] = ar && ar.attributes?.name?.toLowerCase() === name.toLowerCase() ? ar.attributes.artwork?.url || '' : '';
      } catch { break; }
      store.set('artistPhotos', photos);
      if (photos[name]) card.querySelector('.art').insertAdjacentHTML('afterbegin', img(artUrl(photos[name], 160)));
    }
    busy = false;
  };
  const io = new IntersectionObserver(es => {
    for (const e of es) if (e.isIntersecting) { io.unobserve(e.target); queue.push(e.target); }
    pump();
  }, { rootMargin: '200px' });
  for (const c of v.querySelectorAll('.card[data-name]')) if (!(c.dataset.name in photos)) io.observe(c);
}

function localEmpty(v) {
  v.innerHTML = `<div class="empty"><h2>${trad('Ajoutez votre musique')}</h2><div>${trad('Choisissez les dossiers où se trouvent vos fichiers (FLAC, ALAC, WAV, AIFF, MP3, AAC, Vorbis…),<br>ou glissez des fichiers et dossiers dans la fenêtre. Ils restent sur votre ordinateur.')}</div><button class="btn primary" id="addf">${trad('Choisir un dossier')}</button>${local.scanning ? '<div class="spinner"></div>' : ''}</div>`;
  $('#addf').onclick = () => native('local.addFolder');
}

async function loadPlaylists() {
  const box = $('#playlists');
  // liste relue à chaque appel (création, renommage) : pas de copie de moins de 3 min
  uncache('/v1/me/library/playlists?');
  try {
    const all = await apiAll('/v1/me/library/playlists?limit=100', () => { }, 1000);
    apple.playlists = all;
    box.innerHTML = `<a id="newpl"><svg viewBox="0 0 24 24"><path d="M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6z"/></svg><span class="label">${trad('Nouvelle playlist')}</span></a>` + all.map(p => `<a data-pl="${esc(p.id)}" title="${esc(p.attributes?.name)}"><span class="pl-ic">${p.attributes?.artwork ? img(artUrl(p.attributes.artwork, 22)) : PL_ICON}</span><span class="label">${esc(p.attributes?.name)}</span></a>`).join('');
  } catch { box.innerHTML = ''; }
}

// ── playlist : lecture, réorganisation, édition, suggestions ─────────────
async function playlistView(v, id, live) {
  const lib = String(id).startsWith('p.');
  const r = await api(lib ? `/v1/me/library/playlists/${id}` : `/v1/catalog/${apple.sf}/playlists/${id}`);
  if (!live()) return;
  const pl = r.data?.[0]; const a = pl?.attributes || {};
  const editable = lib && a.canEdit !== false && apple.authorized;
  v.innerHTML = `<div class="hero"><div class="cover">${img(artUrl(a.artwork, 230))}</div><div class="info"><div class="muted">${trad('Playlist')}</div><h1 id="pl-name">${esc(a.name)}</h1><div class="muted">${esc(a.curatorName || a.description?.standard || '')}</div><div class="muted" id="pl-count"></div><div class="actions"><button class="btn primary" id="pa">▶ ${trad('Lecture')}</button><button class="btn" id="sh">${trad('Aléatoire')}</button>${editable ? `<button class="btn" id="ed">✎ ${trad('Modifier')}</button>` : ''}</div></div></div><div id="pl-edit"></div><div id="pl-list"></div><div id="pl-sugg"></div>`;
  const play = (items, i) => lib ? playAppleSongs(items, i) : player.playApple({ playlist: id, startWith: i, startPosition: i });
  let list = null, loaded = false;
  const st = { editing: false, name: a.name, added: 0, dirty: false };
  const count = () => { const d = list?.items.reduce((s, t) => s + (t.duration || 0), 0) || 0; $('#pl-count', v).textContent = tradn(list?.items.length || 0, '{n} morceau', '{n} morceaux') + ' · ' + fmtTime(d); };

  /** Enregistre : ajouts d'abord (l'API les place à la fin), puis l'ordre complet. */
  const save = async silent => {
    if (!loaded) throw new Error('playlist incomplète');
    // le nom d'abord : il ne dépend pas des morceaux
    if (st.name !== a.name) {
      await api(`/v1/me/library/playlists/${id}`, { method: 'PATCH', cache: false, body: { attributes: { name: st.name } } });
      a.name = st.name; $('#pl-name', v).textContent = st.name;
      // barre latérale à jour tout de suite : Apple renvoie l'ancien nom quelques secondes
      const p = apple.playlists.find(p => p.id === id); if (p?.attributes) p.attributes.name = st.name;
      const link = $(`#playlists a[data-pl="${CSS.escape(id)}"]`);
      if (link) { link.title = st.name; $('.label', link).textContent = st.name; }
    }
    // ajouts envoyés une seule fois (l'API les place à la fin), même si l'enregistrement est repris
    const unsent = list.items.filter(it => it.pending && !it.sent);
    if (unsent.length) {
      await addToPlaylist(id, unsent.map(s => s.catalogId || s.id));
      unsent.forEach(it => (it.sent = true));
      st.added = 0;
    }
    if (list.items.some(it => it.pending)) {
      // Apple peut mettre un instant à intégrer l'ajout : on attend de retrouver chaque morceau
      let mapped = null;
      for (let tries = 0; tries < 6 && !mapped; tries++) {
        if (tries) await new Promise(r => setTimeout(r, 1500));
        uncache(`/v1/me/library/playlists/${id}`);
        const fresh = (await playlistTracks(id)).map(songOf);
        const used = new Set(list.items.filter(it => !it.pending).map(it => it.id));
        const next = list.items.map(it => {
          if (!it.pending) return it;
          const m = fresh.find(p => !used.has(p.id) && p.catalogId && p.catalogId === it.catalogId);
          if (m) used.add(m.id);
          return m || it;
        });
        if (!next.some(it => it.pending)) mapped = next;
      }
      // morceaux ajoutés mais pas encore visibles : ils restent à la fin, et l'ordre n'est
      // pas envoyé (une liste sans eux les effacerait de la playlist)
      if (!mapped) { st.dirty = true; uncache(`/v1/me/library/playlists/${id}`); if (!silent) toast('Morceaux ajoutés ; ordre à réenregistrer dans un instant'); return; }
      list.update(mapped);
    }
    const data = list.items.filter(it => String(it.id).startsWith('i.')).map(it => ({ id: it.id, type: 'library-songs' }));
    // garde-fou : jamais d'envoi qui viderait ou amputerait la playlist par erreur
    if (data.length < list.items.length) throw new Error('morceaux sans identifiant de bibliothèque');
    await api(`/v1/me/library/playlists/${id}/tracks`, { method: 'PUT', cache: false, body: { data } });
    uncache(`/v1/me/library/playlists/${id}`);
    st.dirty = false;
    if (!silent) toast('Playlist enregistrée');
  };
  const opts = {
    editable,
    onReorder: () => {
      count();
      if (!loaded) { toast('Patientez, la playlist finit de se charger'); return router.render(); }
      if (st.editing) st.dirty = true; else save(true).then(() => toast('Ordre enregistré'), () => toast('Enregistrement impossible'));
    },
    onRemove: i => {
      if (i < 0 || !list) return;
      if (!loaded) return toast('Patientez, la playlist finit de se charger');
      const it = list.items.slice(); const [gone] = it.splice(i, 1); list.update(it); count();
      if (gone?.pending) st.added = Math.max(0, st.added - 1);
      st.dirty = true;
      if (!st.editing) save(true).then(() => toast('Morceau retiré'), () => toast('Échec'));
    },
  };
  const path = lib ? `/v1/me/library/playlists/${id}/tracks?limit=100` : `/v1/catalog/${apple.sf}/playlists/${id}/tracks?limit=100`;
  $('#pa', v).onclick = () => list?.items.length ? play(list.items, 0) : player.playApple({ playlist: id });
  $('#sh', v).onclick = () => list?.items.length ? playAppleSongs(shuffled(list.items), 0) : player.playApple({ playlist: id });
  $('#ed', v)?.addEventListener('click', () => toggleEdit());

  function toggleEdit() {
    if (!list) return;
    st.editing = !st.editing;
    list.box.classList.toggle('editing', st.editing);
    list.head.classList.toggle('editing', st.editing);
    $('#ed', v).textContent = st.editing ? '✓ ' + trad('Terminer') : '✎ ' + trad('Modifier');
    const box = $('#pl-edit', v);
    if (!st.editing) {
      box.innerHTML = '';
      if (st.dirty || st.added || st.name !== a.name) save().catch(() => toast('Enregistrement impossible'));
      return;
    }
    box.innerHTML = `<div class="editbar glass"><input type="text" id="pl-rename" value="${esc(st.name)}" maxlength="100" title="${trad('Nom de la playlist')}">
      <div class="grp"><span class="muted">${trad('Trier :')}</span><span class="chip" data-s="title">${trad('Titre')}</span><span class="chip" data-s="artist">${trad('Artiste')}</span><span class="chip" data-s="album">${trad('Album')}</span><span class="chip" data-s="duration">${trad('Durée')}</span><span class="chip" data-s="shuffle">${trad('Mélanger')}</span><span class="chip" data-s="reverse">${trad('Inverser')}</span></div>
      <input type="search" id="pl-add" placeholder="${trad('Ajouter des morceaux : rechercher un titre, un artiste…')}"></div><div id="pl-results" class="add-results glass" hidden></div>`;
    $('#pl-rename', v).oninput = e => { st.name = e.target.value.trim() || a.name; };
    box.querySelectorAll('[data-s]').forEach(c => c.onclick = () => {
      const k = c.dataset.s; const it = list.items.slice();
      if (k === 'reverse') it.reverse();
      else if (k === 'shuffle') for (let i = it.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); [it[i], it[j]] = [it[j], it[i]]; }
      else it.sort((x, y) => k === 'duration' ? x.duration - y.duration : String(x[k] || '').localeCompare(String(y[k] || ''), undefined, { sensitivity: 'base', numeric: true }));
      list.update(it); st.dirty = true;
    });
    let t = 0;
    $('#pl-add', v).oninput = e => {
      clearTimeout(t);
      const q = e.target.value.trim(), res = $('#pl-results', v);
      if (!q) { res.hidden = true; return; }
      t = setTimeout(async () => {
        const [cat, libr] = await Promise.all([
          api(`/v1/catalog/${apple.sf}/search?term=${encodeURIComponent(q)}&types=songs&limit=10`).catch(() => ({})),
          api(`/v1/me/library/search?term=${encodeURIComponent(q)}&types=library-songs&limit=6`).catch(() => ({})),
        ]);
        const found = [...(libr.results?.['library-songs']?.data || []), ...(cat.results?.songs?.data || [])].map(songOf);
        res.hidden = false;
        res.innerHTML = found.length ? found.map((s, i) => `<div class="qitem" data-k="${i}">${s.art ? img(s.art) : '<div class="ph"></div>'}<div class="c"><div>${esc(s.title)}</div><div class="muted">${esc(s.artist)} · ${esc(s.album)}</div></div><button class="btn small">＋ ${trad('Ajouter')}</button></div>`).join('') : `<div class="empty" style="padding:16px">${trad('Aucun morceau trouvé')}</div>`;
        res.onclick = ev => {
          const row = ev.target.closest('[data-k]'); if (!row) return;
          const s = found[+row.dataset.k];
          st.added++;
          list.update(list.items.concat({ ...s, pending: true, catalogId: s.catalogId || s.id }));
          count();
          row.querySelector('.btn').textContent = '✓ ' + trad('Ajouté');
        };
      }, 280);
    };
  }

  const items = [];
  try {
    await apiAll(path, all => {
      if (!live()) return false;
      for (let k = items.length; k < all.length; k++) items.push(songOf(all[k]));
      if (!list) list = songTable($('#pl-list', v), items, play, opts); else list.update(items);
      count();
    });
  } catch (e) {
    // playlist vide (toute nouvelle) : Apple répond 404 au lieu d'une liste vide
    if (!/HTTP 404/.test(e.message)) throw e;
  }
  if (!live()) return;
  if (!list) { list = songTable($('#pl-list', v), items, play, opts); count(); }
  loaded = true;
  if (live() && editable && list) suggestions(v, id, list, count, st);
}

/** Morceaux suggérés pour une playlist (même service que le lecteur web). */
async function suggestions(v, id, list, count, st) {
  try {
    const r = await api('/v1/me/recommendations/suggested?platform=web&omit[resource]=autos&contexts=playlist-suggested-songs&include[songs]=artists&types=songs&meta=source&limit=20', { method: 'POST', cache: false, body: { targetContent: { id, type: 'library-playlists' } } });
    let songs = r.results?.suggested || (Array.isArray(r.data) && r.data[0]?.type === 'songs' ? r.data : (r.data?.[0]?.relationships?.contents?.data || r.results?.songs?.data || []));
    const have = new Set(list.items.map(s => s.catalogId));
    songs = songs.filter(s => !have.has(s.id)).map(songOf);
    const box = $('#pl-sugg', v);
    if (!songs.length || !box?.isConnected) return;
    box.innerHTML = `<h2>${trad('Suggestions')}</h2><div class="glass add-results">${songs.map((s, i) => `<div class="qitem" data-k="${i}">${s.art ? img(s.art) : '<div class="ph"></div>'}<div class="c"><div>${esc(s.title)}</div><div class="muted">${esc(s.artist)}</div></div><button class="btn small">＋ ${trad('Ajouter')}</button></div>`).join('')}</div>`;
    box.onclick = async e => {
      const row = e.target.closest('[data-k]'); if (!row) return;
      const s = songs[+row.dataset.k];
      if (!e.target.closest('.btn')) return player.playApple({ song: s.id });
      if (row.dataset.added) return;
      row.dataset.added = '1';
      // en cours d'édition : ajout en attente, enregistré avec le reste (rien n'est perdu)
      if (st?.editing || st?.dirty) {
        st.added++;
        list.update(list.items.concat({ ...s, pending: true, catalogId: s.catalogId || s.id }));
        count();
        row.querySelector('.btn').textContent = '✓ ' + trad('Ajouté');
        return;
      }
      try {
        await addToPlaylist(id, [s.id]);
        row.querySelector('.btn').textContent = '✓ ' + trad('Ajouté');
        // la liste affichée doit contenir le morceau (sinon un enregistrement suivant l'effacerait)
        for (let tries = 0; tries < 4; tries++) {
          if (tries) await new Promise(r => setTimeout(r, 1200));
          uncache(`/v1/me/library/playlists/${id}`);
          const fresh = (await playlistTracks(id)).map(songOf);
          if (fresh.some(f => f.catalogId === s.id) || tries === 3) { list.update(fresh); count(); break; }
        }
      } catch { delete row.dataset.added; toast('Ajout impossible'); }
    };
  } catch (e) { log('suggestions : ' + (e.message || e)); }
}

// ── réglages ─────────────────────────────────────────────────────────────
// Logiciels de l'auteur (jeremdevgit), affichés dans les Réglages
const MY_APPS = [
  { t: 'Boîte à outils', os: 'Windows et macOS', d: 'entretien, nettoyage et réglages cachés', url: 'https://github.com/jeremdevgit/boite-a-outils/releases/latest' },
  { t: 'rJb', os: 'Windows', d: 'l\'éclairage RGB de tous vos appareils dans une seule application', url: 'https://github.com/jeremdevgit/rjb/releases/latest' },
  { t: 'Ludothèque', os: 'macOS', d: 'tous vos jeux dans un seul lanceur', url: 'https://github.com/jeremdevgit/ludotheque/releases/latest' },
  { t: 'Suite Libre', os: 'navigateur', d: 'six outils sans compte : QR codes, flou, photos, conversions, moyennes, mots de passe', url: 'https://jeremdevgit.github.io/suite-libre/' },
  { t: 'Batailles de France', os: 'navigateur', d: 'jeu de stratégie en temps réel sur les cartes de cinq villes', url: 'https://github.com/jeremdevgit/batailles-de-france/releases/latest' },
];
const EQ_PRESETS = {
  'Plat': [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'Basses +': [6, 5, 4, 2, 0, 0, 0, 0, 0, 0], 'Aigus +': [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
  'Voix': [-2, -1, 0, 2, 4, 4, 3, 1, 0, -1], 'Rock': [4, 3, 1, -1, -2, -1, 1, 3, 4, 4], 'Électro': [5, 4, 1, 0, -1, 1, 0, 2, 4, 5],
  'Classique': [3, 2, 1, 0, 0, 0, -1, 0, 2, 3], 'Casque': [2, 1, 0, -1, 0, 1, 2, 2, 3, 1], 'Loudness': [5, 3, 0, 0, -1, 0, 0, 1, 3, 4],
};
const FREQS = ['31', '62', '125', '250', '500', '1k', '2k', '4k', '8k', '16k'];
const settings = {
  eq: store.get('eq', { enabled: false, preamp: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }),
  sendEq() { native('audio.eq', { eq: this.eq }); store.set('eq', this.eq); },
  render(v) {
    const eq = this.eq, q = String(store.get('quality', 'auto')), dev = store.get('device', '');
    const tg = (on, attr, label) => `<span class="toggle${on ? ' on' : ''}" ${attr}><i></i>${trad(label)}</span>`;
    const sleepLeft = sleep.until ? Math.ceil((sleep.until - Date.now()) / 60000) : 0;
    v.innerHTML = `<h1>${trad('Réglages')}</h1>
    <div class="card-box glass"><h2 style="margin-top:0">${trad('Langue')}</h2>
      <div class="row-head" style="gap:10px"><span>${trad("Langue de l'interface")}</span><span data-langues></span></div></div>
    <div class="card-box glass"><h2 style="margin-top:0">${trad('Qualité de lecture')}</h2>
      <div class="muted">${trad("Apple Music — le lecteur web d'Apple ne délivre pas de lossless : 256 kb/s est le maximum possible.")}</div>
      <div class="chips" style="margin:10px 0 16px"><span class="chip${q === 'auto' ? ' on' : ''}" data-q="auto">${trad('Automatique')}</span><span class="chip${q === '256' ? ' on' : ''}" data-q="256">${trad('Haute qualité · AAC 256 kb/s')}</span><span class="chip${q === '64' ? ' on' : ''}" data-q="64">${trad('Économie de données · 64 kb/s')}</span></div>
      <div class="muted">${trad("Fichiers de l'ordinateur — décodage sans perte (FLAC, ALAC, WAV, AIFF, CAF) jusqu'en 24 bits / 192 kHz.")}</div>
      <div class="stack" style="margin-top:10px">
        <label class="row-head" style="gap:10px"><span>${trad('Sortie audio')}</span><select id="dev"><option value="">${trad('Sortie par défaut de Windows')}</option>${app.devices.map(d => `<option${d === dev ? ' selected' : ''}>${esc(d)}</option>`).join('')}</select></label>
        ${tg(store.get('normalize', false), 'id="norm"', 'Normaliser le volume (ReplayGain des fichiers)')}
      </div></div>
    <div class="card-box glass"><div class="row-head"><h2 style="margin:0">${trad('Égaliseur')}</h2>${tg(eq.enabled, 'id="eq-on"', eq.enabled ? 'Activé' : 'Désactivé')}</div>
      <div class="muted" style="margin:6px 0 10px">${trad("S'applique aux fichiers de votre ordinateur. Les morceaux d'Apple Music sont protégés et ne peuvent pas être modifiés.")}</div>
      <div class="chips">${Object.keys(EQ_PRESETS).map(n => `<span class="chip" data-p="${n}">${esc(trad(n))}</span>`).join('')}</div>
      <div class="eq">${eq.gains.map((g, i) => `<div class="band"><span>${g > 0 ? '+' : ''}${g}</span><input type="range" min="-12" max="12" step="1" value="${g}" data-b="${i}"><span>${FREQS[i]}</span></div>`).join('')}
        <div class="band" style="margin-left:14px"><span>${eq.preamp}</span><input type="range" min="-12" max="12" step="1" value="${eq.preamp}" data-b="pre"><span>${trad('Gain')}</span></div></div></div>
    <div class="card-box glass"><h2 style="margin-top:0">${trad('Minuterie de mise en veille')}</h2>
      <div class="chips">${[['0', 'Désactivée'], ['15', '15 min'], ['30', '30 min'], ['45', '45 min'], ['60', '1 h'], ['end', 'Fin du morceau']].map(([k, l]) => `<span class="chip${(k === 'end' && sleep.endOfTrack) || (k === '0' && !sleep.until && !sleep.endOfTrack) ? ' on' : ''}" data-sl="${k}">${trad(l)}</span>`).join('')}</div>
      ${sleepLeft ? `<div class="muted" style="margin-top:8px">${trad('Arrêt dans {n} min', { n: sleepLeft })}</div>` : ''}</div>
    <div class="card-box glass"><h2 style="margin-top:0">${trad('Musique sur cet ordinateur')}</h2>
      ${local.folders.map(f => `<div class="folder"><span title="${esc(f)}">${esc(f)}</span><button class="btn small" data-rm="${esc(f)}">${trad('Retirer')}</button></div>`).join('') || `<div class="muted">${trad("Aucun dossier pour l'instant. Vous pouvez aussi glisser des dossiers dans la fenêtre.")}</div>`}
      <div style="display:flex;gap:10px;margin-top:12px;flex-wrap:wrap"><button class="btn primary" id="addf">${trad('Ajouter un dossier')}</button><button class="btn" id="rescan">${trad('Réanalyser')}</button><span class="muted" id="scan-state" style="align-self:center">${tradn(local.tracks.length, '{n} morceau', '{n} morceaux')}</span></div></div>
    <div class="card-box glass"><h2 style="margin-top:0">${trad('Application')}</h2><div class="stack">
      ${tg(app.closeToTray, 'data-cfg="closeToTray"', 'Fermer la fenêtre garde la musique dans la zone de notification')}
      ${tg(app.autostart, 'data-cfg="autostart"', 'Lancer Pomme au démarrage du PC')}
      ${tg(store.get('onlineLyrics', true), 'id="olyr"', 'Chercher les paroles en ligne sur LRCLIB quand Apple n\'en fournit pas (envoie le titre et l\'artiste du morceau)')}
      ${tg(app.gpu, 'data-cfg="gpu"', 'Accélération graphique : défilement fluide à 144 Hz et plus (≈ 50 Mo de plus, au prochain lancement)')}
    </div><div class="muted" style="margin-top:10px">${trad('Raccourcis : Espace lecture/pause · Ctrl+←/→ morceau précédent/suivant · ←/→ ±5 s · ↑/↓ volume · Ctrl+F rechercher · L paroles plein écran · M mini-lecteur · Échap fermer')}</div></div>
    <div class="card-box glass"><h2 style="margin-top:0">Discord</h2><div class="stack">
      ${tg(app.discord, 'data-cfg="discord"', 'Afficher le morceau en cours sur Discord (titre, artiste, pochette, progression ; effacé en pause)')}
      <details class="muted"><summary style="cursor:pointer">${trad('Identifiant de votre application Discord')}</summary>
        <label style="display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin-top:8px">${trad("ID d'application")}
          <input id="dcid" class="field" inputmode="numeric" maxlength="24" placeholder="${trad('obligatoire pour activer le statut')}" value="${esc(app.discordId)}" style="width:220px"></label>
        <div style="margin-top:6px">${trad("Le nom affiché (« Écoute … ») est celui de l'application sur {lien}.", { lien: '<a id="dcdev" style="text-decoration:underline">discord.com/developers</a>' })}</div></details>
    </div></div>
    <div class="card-box glass"><h2 style="margin-top:0">${trad('Compte')}</h2><button class="btn" id="acct">${trad(apple.authorized ? "Se déconnecter d'Apple Music" : 'Se connecter à Apple Music')}</button></div>
    <div class="card-box glass"><h2 style="margin-top:0">${trad('À propos')}</h2>
      <div>${trad('Pomme {version} — lecteur de musique libre (GPL-3.0), créé par <b>jeremdevgit</b>.', { version: esc(app.version) })}</div>
      <div class="muted">${trad("Pomme n'est ni affilié ni approuvé par Apple. Apple Music est une marque d'Apple Inc. Un abonnement Apple Music est nécessaire pour écouter son catalogue.")}</div>
      <div class="muted">${trad('Paroles : Apple Music et {lien}.', { lien: '<a id="lrclib" style="text-decoration:underline">LRCLIB</a>' })}</div></div>
    <div class="card-box glass"><h2 style="margin-top:0">${trad('Mes autres logiciels')}</h2>
      <div class="muted" style="margin-bottom:10px">${trad('Libres, gratuits et sans publicité. Chaque bouton ouvre la page de téléchargement officielle sur GitHub.')}</div>
      <div class="stack">${MY_APPS.map((m, i) => `<div class="folder"><span><b>${esc(m.t)}</b> <span class="muted">· ${esc(trad(m.os))} — ${esc(trad(m.d))}</span></span><button class="btn small" data-app="${i}">${trad('Télécharger')}</button></div>`).join('')}</div>
      <div style="margin-top:12px"><button class="btn" id="gh-profile">${trad('Tous mes logiciels sur GitHub')}</button></div></div>`;
    menusLangues(v);
    $('#eq-on').onclick = () => { eq.enabled = !eq.enabled; this.sendEq(); this.render(v); };
    $$('.chip[data-p]', v).forEach(c => c.onclick = () => { eq.gains = EQ_PRESETS[c.dataset.p].slice(); eq.enabled = true; this.sendEq(); this.render(v); });
    let t = 0;
    $$('.eq input', v).forEach(inp => inp.oninput = () => {
      const b = inp.dataset.b, val = +inp.value;
      if (b === 'pre') eq.preamp = val; else eq.gains[+b] = val;
      inp.previousElementSibling.textContent = (val > 0 ? '+' : '') + val;
      clearTimeout(t); t = setTimeout(() => this.sendEq(), 60);
    });
    $$('.chip[data-q]', v).forEach(c => c.onclick = () => { store.set('quality', c.dataset.q); applyBitrate(); this.render(v); ui.nowPlaying(); });
    $('#dev').onchange = e => output.set(e.target.value);
    $('#olyr').onclick = () => { store.set('onlineLyrics', !store.get('onlineLyrics', true)); this.render(v); };
    $('#norm').onclick = () => { store.set('normalize', !store.get('normalize', false)); this.render(v); };
    $$('.chip[data-sl]', v).forEach(c => c.onclick = () => { sleep.set(c.dataset.sl === 'end' ? 'end' : +c.dataset.sl); this.render(v); });
    $('#addf').onclick = () => native('local.addFolder');
    $('#rescan').onclick = () => { native('local.rescan'); local.scanning = true; $('#scan-state').textContent = trad('Analyse…'); };
    $$('[data-rm]', v).forEach(b => b.onclick = () => native('local.removeFolder', { folder: b.dataset.rm }));
    $$('[data-cfg]', v).forEach(el => el.onclick = () => { const k = el.dataset.cfg; app[k] = !app[k]; native('config', { [k]: app[k] }); this.render(v); });
    $('#dcid').onchange = e => { const id = e.target.value.replace(/\D/g, '').slice(0, 24); e.target.value = id; app.discordId = id; native('config', { discordId: id }); toast(id ? 'Discord : application enregistrée' : 'Discord : application de Pomme rétablie'); };
    $('#dcdev').onclick = () => native('openExternal', { url: 'https://discord.com/developers/applications' });
    $('#acct').onclick = signIn;
    $('#lrclib').onclick = () => native('openExternal', { url: 'https://lrclib.net' });
    $$('[data-app]', v).forEach(b => b.onclick = () => native('openExternal', { url: MY_APPS[+b.dataset.app].url }));
    $('#gh-profile').onclick = () => native('openExternal', { url: 'https://github.com/jeremdevgit' });
    if (!app.devices.length) native('audio.devices');
  },
};

// ── messages du code natif ───────────────────────────────────────────────
window.pomme = {
  recv(m) {
    switch (m.type) {
      case 'config':
        Object.assign(app, { closeToTray: m.closeToTray, autostart: m.autostart, gpu: m.gpu, discord: m.discord, discordId: m.discordId || '', version: m.version });
        if (router.view === 'settings') router.render();
        break;
      case 'devices': {
        const list = m.list || [];
        const changed = list.join('\n') !== app.devices.join('\n');
        app.devices = list;
        if (output.pendingMenu) output.showMenu();
        else if (changed && router.view === 'settings') router.render();
        break;
      }
      case 'library':
        local.tracks = m.tracks || []; local.folders = m.folders || [];
        for (const t of local.tracks) local.byId.set(t.id, t);
        local.scanning = false;
        session.restoreLocal();
        if (router.view.startsWith('local') || router.view === 'settings' || (router.view === 'home' && !apple.ok)) router.render();
        break;
      case 'dropped':
        for (const t of m.tracks || []) local.byId.set(t.id, t);
        if (m.ids?.length) { player.playLocal(m.ids, 0); toast(m.ids.length > 1 ? trad('{n} fichiers en lecture', { n: m.ids.length }) : 'Lecture du fichier'); }
        break;
      case 'scanProgress':
        local.scanning = true;
        { const s = $('#scan-state'); if (s) s.textContent = trad('Analyse… {n} fichiers', { n: m.count }); }
        break;
      case 'local.loaded':
        player.dur = m.duration; ui.duration = m.duration; for (const e of $$('.t-dur')) e.textContent = fmtTime(m.duration);
        break;
      case 'local.position':
        player.pos = m.position; player.posAt = performance.now();
        if (player.src === 'local') player.onTime();
        break;
      case 'local.playing':
        player.playing = m.playing; player.posAt = performance.now();
        if (player.src === 'local') player.onState();
        break;
      case 'local.ended':
        if (player.src === 'local') player.ended();
        break;
      case 'local.error':
        toast(tradErr(m.message));
        if (player.src === 'local' && player.index < player.order.length - 1) setTimeout(() => player.next(), 800);
        break;
      case 'lyrics':
        lyrics.receive(m);
        break;
      case 'auth':
        $('#auth').hidden = !m.open;
        if (!m.open) setTimeout(onAuth, 300);
        break;
      case 'mediaKey':
        if (m.action === 'next') player.next();
        else if (m.action === 'previous') player.prev();
        else if (m.action === 'stop') player.pause();
        else if (m.action === 'play' && !player.playing) player.toggle();
        else if (m.action === 'pause' && player.playing) player.toggle();
        else if (m.action === 'toggle') player.toggle();
        break;
    }
  },
};

// ── démarrage ────────────────────────────────────────────────────────────
function bind() {
  bindCards($('#view'));
  $('#nav').addEventListener('click', e => {
    const a = e.target.closest('a');
    if (!a) return;
    if (a.id === 'newpl') {
      ask('Nom de la nouvelle playlist').then(name => name && api('/v1/me/library/playlists', { method: 'POST', cache: false, body: { attributes: { name: name.slice(0, 100) } } }).then(r => { toast('Playlist créée'); loadPlaylists(); setTimeout(loadPlaylists, 4000); const id = r.data?.[0]?.id; if (id) router.go('playlist', id); }, () => toast('Création impossible')));
      return;
    }
    if (a.dataset.pl) return router.go('playlist', a.dataset.pl);
    if (a.dataset.view) router.go(a.dataset.view);
  });
  let st = 0;
  $('#q').addEventListener('input', e => { clearTimeout(st); st = setTimeout(() => router.go('search', e.target.value, router.view !== 'search'), 300); });
  $('#account').onclick = signIn;
  $('#settings-btn').onclick = () => router.go('settings');
  $('#menu-btn').onclick = () => document.body.classList.toggle('drawer');
  $('#scrim').onclick = () => document.body.classList.remove('drawer');
  $('#back-btn').onclick = () => router.back();
  $('#play').onclick = () => player.toggle();
  $('#next').onclick = () => player.next();
  $('#prev').onclick = () => player.prev();
  $('#shuffle').onclick = () => player.setShuffle(!player.shuffle);
  $('#repeat').onclick = () => player.setRepeat([2, 0, 1][player.repeat]);
  $('#lyrics-btn').onclick = () => panel.open('lyrics');
  $('#queue-btn').onclick = () => panel.open('queue');
  $('#mini-btn').onclick = () => mini.set(true);
  $('#out-btn').onclick = e => output.menu(e);
  $('#mini-exit').onclick = () => mini.set(false);
  $('#mini').addEventListener('dblclick', e => { if (!e.target.closest('button')) mini.set(false); });
  $('#np-cover').onclick = () => fullscreen.open();
  $('#np-close').onclick = () => fullscreen.close();
  $('#panel-close').onclick = () => panel.close();
  $('#auth-cancel').onclick = () => native('auth.close');
  $$('#panel .tabs button').forEach(b => b.onclick = () => { panel.view = null; panel.open(b.dataset.panel); });
  // boutons de la vue plein écran et du mini-lecteur
  const acts = { play: () => player.toggle(), next: () => player.next(), prev: () => player.prev(), shuffle: () => player.setShuffle(!player.shuffle), repeat: () => player.setRepeat([2, 0, 1][player.repeat]) };
  for (const root of [$('#np'), $('#mini')]) root.addEventListener('click', e => { const b = e.target.closest('[data-act]'); if (b) acts[b.dataset.act](); });
  const lyricClick = e => {
    const p = e.target.closest('.lyrics .l');
    if (p && lyrics.data && !lyrics.data.plain) { player.seek(lyrics.data.lines[+p.dataset.i].t); return true; }
    return false;
  };
  $('#np-lyrics').addEventListener('click', lyricClick);
  $('#np-queue').addEventListener('click', queueClick);
  $$('#np .np-tabs button').forEach(b => b.onclick = () => fullscreen.setTab(b.dataset.np));
  $('#panel-body').addEventListener('click', e => { if (!lyricClick(e)) queueClick(e); });
  // changement de taille, d'orientation ou d'écran (l'échelle selon l'écran est réglée par Pomme côté natif)
  let rz = 0;
  addEventListener('resize', () => { cancelAnimationFrame(rz); rz = requestAnimationFrame(() => lyrics.recenter()); });
  const vol = $('#vol');
  vol.value = player.volume; vol.style.setProperty('--v', player.volume * 100 + '%');
  vol.oninput = () => player.setVolume(+vol.value);
  $('.vol').addEventListener('wheel', e => { e.preventDefault(); player.setVolume(player.volume - Math.sign(e.deltaY) * 0.05); }, { passive: false });
  // fenêtre réduite : le curseur est masqué, l'icône l'ouvre au-dessus de la barre
  $('.vol-ico').addEventListener('click', () => $('.vol').classList.toggle('open'));
  addEventListener('pointerdown', e => { if (!e.target.closest('.vol')) $('.vol').classList.remove('open'); });
  for (const bar of $$('.bar')) bar.addEventListener('click', e => { const r = bar.getBoundingClientRect(); const d = ui.duration || player.current()?.duration || 0; if (d) player.seek(((e.clientX - r.left) / r.width) * d); });
  addEventListener('keydown', e => {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') { if (e.key === 'Escape') e.target.blur(); return; }
    if (e.key === 'Escape') { if (fullscreen.on) fullscreen.close(); else if (mini.on) mini.set(false); else menu.close(); }
    else if (e.code === 'Space') { e.preventDefault(); player.toggle(); }
    else if (e.ctrlKey && e.key.toLowerCase() === 'f') { e.preventDefault(); $('#q').focus(); }
    else if (e.key === 'ArrowRight' && e.ctrlKey) player.next();
    else if (e.key === 'ArrowLeft' && e.ctrlKey) player.prev();
    else if (e.key === 'ArrowRight') player.seek(player.position() + 5);
    else if (e.key === 'ArrowLeft') player.seek(player.position() - 5);
    else if (e.key === 'ArrowUp') { e.preventDefault(); player.setVolume(player.volume + 0.05); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); player.setVolume(player.volume - 0.05); }
    else if (e.key.toLowerCase() === 'l' && !e.ctrlKey) fullscreen.on ? fullscreen.close() : fullscreen.open();
    else if (e.key.toLowerCase() === 'm' && !e.ctrlKey) mini.set(!mini.on);
    else if (e.key === 'F12') native('devtools');
  });
  addEventListener('mouseup', e => { if (e.button === 3) router.back(); });
  document.addEventListener('visibilitychange', () => { if (!document.hidden) { ui.progress(player.position(), true); lyrics.syncNow(); lyrics.tick(); } });
}

bind();
ui.modes();
ui.playState();
native('audio.volume', { volume: player.volume });
// sortie choisie précédemment : sans nouvelle demande d'autorisation au démarrage
if (output.name) { native('audio.device', { name: output.name }); output.apply(false); }
settings.sendEq();
const appleReady = initApple();
router.go(store.get('view', 'home'), null, false);
requestAnimationFrame(() => native('ready', { lang: LANG }));
