// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '..');
const ui = fs.existsSync(path.join(root, 'ui')) ? 'ui' : 'app/static';
const prefix = ui === 'ui' ? '' : '/static/';
const source = fs.readFileSync(path.join(root, ui, 'textes-chargeur.js'), 'utf8');
const languages = ['fr', 'en', 'es', 'de', 'it', 'pt', 'zh', 'ja'];
for (const lang of languages.concat(['<img src=x onerror=alert(1)>', '__proto__', 'en.js\" onload=alert(1)'])) {
  const writes = [];
  vm.runInNewContext(source, {
    URLSearchParams, location: {search: '?lang=' + encodeURIComponent(lang)},
    localStorage: {getItem: () => '<script>alert(1)</script>'},
    navigator: {languages: ['en-US']},
    document: {currentScript: {src: 'https://example.test/\" onerror=alert(1)/loader.js'}, write: s => writes.push(s)}
  });
  const expected = languages.includes(lang) ? lang : 'en';
  assert.deepEqual(writes, expected === 'fr' ? [] : [`<script src="${prefix}textes/${expected}.js"></script>`]);
}
async function networkTest() {
  const file = path.join(root, ui, 'reseau.js');
  if (!fs.existsSync(file)) return;
  let socket;
  class FakeWebSocket {
    constructor() { socket = this; this.readyState = 1; }
    send() {} close() {}
  }
  const window = {};
  vm.runInNewContext(fs.readFileSync(file, 'utf8'), {
    window, document: {addEventListener() {}, hidden: false}, WebSocket: FakeWebSocket, location: {protocol: 'https:', host: 'example.test'},
    localStorage: {getItem: () => 'a'.repeat(48)}, setInterval: () => 1, clearInterval() {},
  });
  const room = await window.claude.use('room');
  const joined = room.join('security-test');
  socket.onmessage({data: JSON.stringify({t:'bienvenue', moi:'me', by:'demo', pairs:[]})});
  const salon = await joined;
  let messages = 0;
  salon.on('__proto__', () => messages++);
  socket.onmessage({data: JSON.stringify({t:'pairs', arrives:[{p:'__proto__', by:'demo-attacker', pr:{inherited:'must not be enumerated'}}]})});
  const peers = salon.peers();
  assert.equal(peers.length, 2);
  assert.equal(peers[1].peer, '__proto__');
  socket.onmessage({data: JSON.stringify({t:'msg', s:'__proto__', p:'demo', d:{}})});
  assert.equal(messages, 1);
}
networkTest().then(() => console.log('Translation injection and network prototype regressions passed')).catch(e => {console.error(e); process.exitCode = 1;});
