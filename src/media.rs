// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Contrôles multimédia de Windows (touches du clavier, volume de Windows,
//! écran de verrouillage) : titre, artiste, pochette et état de lecture.

use serde_json::Value;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};
use std::time::Duration;

pub struct Controls {
    inner: Option<MediaControls>,
    last: String,
}

impl Controls {
    pub fn new(hwnd: isize, on_action: impl Fn(&str) + Send + 'static) -> Controls {
        let config = PlatformConfig {
            dbus_name: "pomme",
            display_name: "Pomme",
            hwnd: Some(hwnd as *mut std::ffi::c_void),
        };
        let inner = MediaControls::new(config).ok().and_then(|mut c| {
            c.attach(move |e| {
                let a = match e {
                    MediaControlEvent::Play => "play",
                    MediaControlEvent::Pause => "pause",
                    MediaControlEvent::Toggle => "toggle",
                    MediaControlEvent::Next => "next",
                    MediaControlEvent::Previous => "previous",
                    MediaControlEvent::Stop => "stop",
                    _ => return,
                };
                on_action(a);
            })
            .ok()?;
            Some(c)
        });
        Controls { inner, last: String::new() }
    }

    /// `{title, artist, album, art, duration, playing, position}`
    pub fn update(&mut self, v: &Value) {
        let Some(c) = self.inner.as_mut() else { return };
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        let key = format!("{}|{}|{}|{}", s("title"), s("artist"), s("album"), s("art"));
        if key != self.last {
            self.last = key;
            let (t, a, al, art) = (s("title"), s("artist"), s("album"), s("art"));
            let _ = c.set_metadata(MediaMetadata {
                title: Some(&t),
                artist: Some(&a),
                album: Some(&al),
                cover_url: (!art.is_empty()).then_some(art.as_str()),
                duration: v["duration"].as_f64().map(Duration::from_secs_f64),
            });
        }
        let progress = v["position"].as_f64().map(|p| MediaPosition(Duration::from_secs_f64(p.max(0.0))));
        let _ = c.set_playback(match v["playing"].as_bool() {
            Some(true) => MediaPlayback::Playing { progress },
            Some(false) => MediaPlayback::Paused { progress },
            None => MediaPlayback::Stopped,
        });
    }
}
