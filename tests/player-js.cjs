// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(require('node:path').join(__dirname, '../ui/app.js'), 'utf8');
const nodes = Array.from({length: 4}, (_, i) => ({type: i % 2 ? 'number' : 'range', value: '', style: {setProperty(){}}}));
const calls = [], notices = [];
let stored = {};
const context = vm.createContext({
  console, setTimeout: fn => { fn(); }, clearTimeout(){}, performance: {now: () => 0},
  apple: {ok: true, authorized: true, music: null}, store: {get: (k,d) => stored[k] ?? d, set: (k,v) => stored[k] = v},
  native: (cmd, data) => calls.push([cmd,data]), toast: s => notices.push(s), log(){},
  $: () => nodes[0], $$: () => nodes, trad: s => s, tradErr: s => s, local: {byId: new Map()},
  ui: {modes(){}, progress(){}}, lyrics: {syncNow(){}},
});
const take = (start, end) => source.slice(source.indexOf(start), source.indexOf(end, source.indexOf(start)));
vm.runInContext(take('const isAbort =', 'addEventListener(\'error\''), context);
vm.runInContext(take('const player =', '// ── sortie audio'), context);
vm.runInContext(take('function filterSongs(', 'function songTable('), context);
const player = vm.runInContext('player', context);
const filter = vm.runInContext('filterSongs', context);
const tick = () => new Promise(r => setImmediate(r));
async function main() {
  let active = 0, maxActive = 0, release;
  const music = context.apple.music = {
    isPlaying: false,
    async setQueue(desc) {
      maxActive = Math.max(maxActive, ++active); calls.push(['queue', desc]);
      assert.equal(desc.startPlaying, false);
      if (desc.song === 'old') await new Promise(r => release = r);
      active--;
    },
    async play() { calls.push(['play']); music.isPlaying = true; },
    async pause() { calls.push(['pause']); music.isPlaying = false; },
  };
  const old = player.playApple({song: 'old'}); await tick();
  const latest = player.playApple({song: 'latest'}); release();
  await Promise.all([old, latest]);
  assert.equal(maxActive, 1, 'queue changes never overlap');
  assert.equal(calls.filter(c => c[0] === 'play').length, 1, 'superseded request never plays');
  let releaseLoading;
  music.setQueue = async () => { await new Promise(r => releaseLoading = r); };
  music.isPlaying = false;
  const loading = player.playApple({song: 'loading'}); await tick();
  const playClick = player.toggle();
  releaseLoading(); await Promise.all([loading, playClick]);
  assert.equal(music.isPlaying, true, 'play click during queue loading does not pause the new song');
  assert.equal(calls.filter(c => c[0] === 'pause').length, 0);
  music.setQueue = async () => {};
  let attempts = 0;
  music.play = async () => { if (++attempts < 3) throw {description: 'The play request was interrupted by a call to pause()'}; music.isPlaying = true; };
  await player.playApple({song: 'recover'});
  assert.equal(attempts, 3); assert.equal(notices.length, 0);
  music.play = async () => { throw {name: 'AbortError'}; };
  await player.playApple({song: 'fail'});
  assert.equal(notices.length, 1, 'persistent interruption produces an actionable notice');
  music.play = async () => { music.isPlaying = true; };
  music.isPlaying = false; await player.toggle(); assert.equal(music.isPlaying, true, 'play still works after a failed request');
  music.setQueue = async () => { player.seq++; player.src = 'local'; };
  let unexpected = false; music.play = async () => unexpected = true;
  await player.playApple({song: 'cancel'}); assert.equal(unexpected, false);
  for (const [input, expected] of [[0,0], [1,1], [-4,0], [9,1], [0.42,0.42]]) {
    player.setVolume(input); assert.equal(player.volume, expected);
    assert.equal(Number(nodes[0].value), expected); assert.equal(Number(nodes[1].value), Math.round(expected*100));
  }
  player.setVolume(NaN); assert.equal(player.volume, .42);
  const items = [{id:'1', title:'Été', artist:'Alpha', album:'Z', duration:20}, {id:'2',title:'Autre',artist:'Été',album:'B',duration:10}, {id:'3',title:'Fin',artist:'Beta',album:'A',duration:30}];
  assert.deepEqual(Array.from(filter(items,'ete','title'), x => x.id), ['1']);
  assert.deepEqual(Array.from(filter(items,'ete','artist'), x => x.id), ['2']);
  assert.deepEqual(Array.from(filter(items,'ete alpha','all'), x => x.id), ['1']);
  assert.deepEqual(Array.from(filter(items,'','all','album'), x => x.id), ['3','2','1']);
  assert.deepEqual(items.map(x=>x.id), ['1','2','3'], 'sorting a view does not mutate the playlist');
  assert.equal(filter(items,'<script>','all').length, 0);
  console.log('Playback concurrency, interrupted play recovery, volume bounds and playlist view regressions passed');
}
main().catch(e => { console.error(e); process.exitCode = 1; });
