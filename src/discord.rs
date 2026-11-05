// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Discord Rich Presence : « Écoute Pomme » avec titre, artiste, pochette et
//! progression, par le canal local de Discord (\\.\pipe\discord-ipc-N).
//! Rien ne quitte le PC si Discord n'est pas lancé ; en pause, le statut est effacé.

use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

enum Msg {
    Track(Value),
    Config(bool, String),
}

pub struct Presence {
    tx: Sender<Msg>,
}

impl Presence {
    pub fn new(on: bool, client_id: String) -> Presence {
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new().name("discord".into()).spawn(move || run(rx, on && !client_id.is_empty(), client_id));
        Presence { tx }
    }

    /// Message `media` de l'interface : titre, artiste, album, pochette, durée, position, lecture.
    pub fn update(&self, v: &Value) {
        let _ = self.tx.send(Msg::Track(v.clone()));
    }

    pub fn configure(&self, on: bool, client_id: String) {
        let _ = self.tx.send(Msg::Config(on && !client_id.is_empty(), client_id));
    }
}

struct Conn(File);

impl Conn {
    fn open(client_id: &str) -> Option<Conn> {
        (0..10).find_map(|i| {
            let f = OpenOptions::new().read(true).write(true).open(format!(r"\\?\pipe\discord-ipc-{i}")).ok()?;
            let mut c = Conn(f);
            c.send(0, &json!({ "v": 1, "client_id": client_id })).ok()?;
            (c.recv()?["evt"] == "READY").then_some(c)
        })
    }

    fn send(&mut self, op: u32, v: &Value) -> std::io::Result<()> {
        let body = v.to_string();
        let mut buf = Vec::with_capacity(8 + body.len());
        buf.extend(op.to_le_bytes());
        buf.extend((body.len() as u32).to_le_bytes());
        buf.extend(body.as_bytes());
        self.0.write_all(&buf)
    }

    fn recv(&mut self) -> Option<Value> {
        let mut head = [0u8; 8];
        self.0.read_exact(&mut head).ok()?;
        let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        if len > 1 << 20 {
            return None;
        }
        let mut body = vec![0; len];
        self.0.read_exact(&mut body).ok()?;
        serde_json::from_slice(&body).ok()
    }

    /// `activity` nul : efface le statut.
    fn set(&mut self, activity: &Value) -> bool {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let msg = json!({ "cmd": "SET_ACTIVITY", "args": { "pid": std::process::id(), "activity": activity }, "nonce": nonce.to_string() });
        self.send(1, &msg).is_ok() && self.recv().is_some()
    }
}

/// Discord exige 2 à 128 caractères par champ.
fn field(s: &str) -> String {
    let s: String = s.trim().chars().take(128).collect();
    if s.chars().count() < 2 { format!("{s}\u{2800}") } else { s }
}

fn activity(v: &Value) -> Value {
    let title = v["title"].as_str().unwrap_or("");
    if !v["playing"].as_bool().unwrap_or(false) || title.is_empty() {
        return Value::Null;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let pos = (v["position"].as_f64().unwrap_or(0.0).max(0.0) * 1000.0) as i64;
    let dur = (v["duration"].as_f64().unwrap_or(0.0).max(0.0) * 1000.0) as i64;
    // pochette Apple (URL publique) ; les fichiers locaux montrent l'icône « pomme » de l'application Discord
    let art = v["art"].as_str().filter(|a| a.starts_with("https://") && a.len() < 256).map(|a| a.replace("bb.webp", "bb.jpg"));
    let mut assets = match art {
        // pochette en grand, petit logo Pomme dans le coin
        Some(url) => json!({ "large_image": url, "small_image": "pomme", "small_text": "Pomme" }),
        None => json!({ "large_image": "pomme" }),
    };
    if let Some(album) = v["album"].as_str().filter(|a| !a.trim().is_empty()) {
        assets["large_text"] = field(album).into();
    }    let artist = v["artist"].as_str().filter(|s| !s.trim().is_empty());
    let album = v["album"].as_str().filter(|s| !s.trim().is_empty());
    // le statut affiche « Écoute <titre · artiste> » au lieu du nom de l'application
    let line = match artist {
        Some(ar) => format!("{} · {}", title.trim(), ar.trim()),
        None => title.to_string(),
    };
    let mut a = json!({
        "type": 2,
        "status_display_type": 2,
        "details": field(&line),
        // l'album figure déjà sous la pochette (large_text)
        "state": field(artist.or(album).unwrap_or("Artiste inconnu")),
        "assets": assets,
    });
    if dur > 0 {
        a["timestamps"] = json!({ "start": now - pos, "end": now - pos + dur });
    }
    a
}

/// Identité d'un statut, sans la dérive de quelques millisecondes de la position.
fn key(a: &Value) -> String {
    let start = a["timestamps"]["start"].as_i64().unwrap_or(0) / 2000;
    format!("{}\u{1}{}\u{1}{}\u{1}{}", a["details"], a["state"], a["assets"]["large_image"], start)
}

fn run(rx: Receiver<Msg>, mut on: bool, mut client_id: String) {
    let mut conn: Option<Conn> = None;
    let mut track = Value::Null;
    let mut sent: Option<String> = None;
    let mut next_try = Instant::now();
    loop {
        let mut msg = match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(m) => Some(m),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        // une rafale de mises à jour (changement de morceau) ne donne qu'un envoi
        if msg.is_some() {
            std::thread::sleep(Duration::from_millis(400));
        }
        loop {
            match msg.take() {
                Some(Msg::Track(v)) => track = v,
                Some(Msg::Config(o, id)) => {
                    if id != client_id {
                        conn = None;
                        sent = None;
                        next_try = Instant::now();
                    }
                    on = o;
                    client_id = id;
                }
                None => {}
            }
            match rx.try_recv() {
                Ok(m) => msg = Some(m),
                Err(_) => break,
            }
        }

        let want = if on && !client_id.is_empty() { activity(&track) } else { Value::Null };
        let k = if want.is_null() { String::new() } else { key(&want) };
        if sent.as_deref() == Some(k.as_str()) {
            continue;
        }
        if conn.is_none() {
            if want.is_null() || Instant::now() < next_try {
                continue;
            }
            conn = Conn::open(&client_id);
            if conn.is_none() {
                next_try = Instant::now() + Duration::from_secs(20);
                continue;
            }
        }
        if conn.as_mut().is_some_and(|c| c.set(&want)) {
            sent = Some(k);
        } else {
            conn = None;
            sent = None;
        }
        if want.is_null() && !on {
            conn = None;
        }
    }
}
