// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Paroles : fichier .lrc ou étiquette pour les morceaux locaux, sinon LRCLIB
//! (base de paroles synchronisées libre et gratuite, https://lrclib.net).

use serde_json::{json, Value};
use std::time::Duration;

pub struct LyricsRequest {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub local_path: Option<String>,
    pub key: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .user_agent(concat!("Pomme/", env!("CARGO_PKG_VERSION"), " (lecteur de musique libre)"))
        .build()
}

fn is_synced(s: &str) -> bool {
    s.lines().filter(|l| l.trim_start().starts_with('[') && l.contains(':')).count() >= 3
}

/// `{ synced, plain, source }` (synced = texte LRC horodaté)
pub fn lyrics(r: &LyricsRequest) -> Value {
    if let Some(p) = &r.local_path {
        if let Some(text) = crate::library::local_lyrics(p) {
            let synced = is_synced(&text);
            return json!({
                "synced": if synced { Some(&text) } else { None },
                "plain": if synced { None } else { Some(&text) },
                "source": "fichier",
            });
        }
    }
    if r.title.is_empty() || r.title.len() > 300 || r.artist.len() > 300 {
        return Value::Null;
    }
    let a = agent();
    let mut req = a
        .get("https://lrclib.net/api/get")
        .query("track_name", &r.title)
        .query("artist_name", &r.artist);
    if !r.album.is_empty() {
        req = req.query("album_name", &r.album);
    }
    if r.duration > 0.0 {
        req = req.query("duration", &format!("{}", r.duration.round() as u64));
    }
    let found = req.call().ok().and_then(|res| res.into_string().ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()));
    let found = found.filter(|v| v["syncedLyrics"].is_string() || v["plainLyrics"].is_string()).or_else(|| {
        // recherche plus souple (titre et artiste seulement)
        a.get("https://lrclib.net/api/search")
            .query("track_name", &r.title)
            .query("artist_name", &r.artist)
            .call()
            .ok()
            .and_then(|res| res.into_string().ok().and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok()))
            .and_then(|list| {
                list.iter()
                    .find(|v| v["syncedLyrics"].is_string())
                    .or(list.first())
                    .cloned()
            })
    });
    match found {
        Some(v) => json!({
            "synced": v["syncedLyrics"],
            "plain": v["plainLyrics"],
            "source": "LRCLIB",
        }),
        None => Value::Null,
    }
}
