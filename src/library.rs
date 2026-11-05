// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Bibliothèque de fichiers locaux : analyse des dossiers choisis, lecture des
//! étiquettes (titre, artiste, album, format), pochettes et paroles intégrées.
//! L'index est gardé dans un fichier JSON ; seuls les fichiers nouveaux ou
//! modifiés sont relus lors d'une nouvelle analyse.

use lofty::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const EXTENSIONS: &[&str] = &[
    "flac", "mp3", "mp2", "mp1", "m4a", "m4b", "mp4", "aac", "alac", "wav", "wave", "aif", "aiff", "aifc", "ogg", "oga", "mka", "webm", "caf",
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalTrack {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: u32,
    pub track: u32,
    pub disc: u32,
    pub duration: f64,
    pub rate: u32,
    pub bits: u8,
    pub bitrate: u32,
    pub format: String,
    pub lossless: bool,
    /// correction ReplayGain du morceau (dB), 0 si absente
    pub gain: f32,
    pub art: bool,
    /// date de modification (secondes) pour ne relire que ce qui a changé
    pub mtime: u64,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub folders: Vec<String>,
    pub tracks: Vec<LocalTrack>,
    /// fichiers glissés dans la fenêtre : lus sans être ajoutés à la bibliothèque
    #[serde(skip)]
    pub extra: Vec<LocalTrack>,
}

fn data_dir() -> PathBuf {
    crate::config::dir()
}

fn index_path() -> PathBuf {
    data_dir().join("library.json")
}

impl Library {
    pub fn load() -> Library {
        std::fs::read(index_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = index_path();
        let _ = std::fs::create_dir_all(data_dir());
        if let Ok(j) = serde_json::to_vec(self) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, j).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    pub fn find(&self, id: &str) -> Option<&LocalTrack> {
        self.tracks.iter().chain(self.extra.iter()).find(|t| t.id == id)
    }

    /// Analyse les dossiers ; `progress` reçoit le nombre de fichiers traités.
    pub fn scan(&mut self, progress: &dyn Fn(usize)) {
        let known: HashMap<String, LocalTrack> = self.tracks.drain(..).map(|t| (t.path.clone(), t)).collect();
        let mut files = Vec::new();
        for f in &self.folders {
            walk(Path::new(f), &mut files, 0);
        }
        files.sort();
        files.dedup();
        let mut out = Vec::with_capacity(files.len());
        for (n, path) in files.iter().enumerate() {
            let p = path.to_string_lossy().to_string();
            let mtime = std::fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            match known.get(&p) {
                Some(t) if t.mtime == mtime => {
                    // une pochette a pu être ajoutée au dossier depuis la dernière analyse
                    let mut t = t.clone();
                    if !t.art {
                        t.art = folder_cover(path).is_some();
                    }
                    out.push(t);
                }
                _ => {
                    // étiquettes malformées : le fichier est ignoré, l'analyse continue
                    let read = std::panic::catch_unwind(|| read_track(path, mtime)).ok().flatten();
                    if let Some(t) = read {
                        out.push(t);
                    }
                }
            }
            if n % 50 == 0 {
                progress(n);
            }
        }
        out.sort_by(|a, b| {
            (a.album_artist.to_lowercase(), a.album.to_lowercase(), a.disc, a.track)
                .cmp(&(b.album_artist.to_lowercase(), b.album.to_lowercase(), b.disc, b.track))
        });
        self.tracks = out;
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 12 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&p, out, depth + 1),
            Ok(t) if t.is_file() => {
                let ok = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| EXTENSIONS.contains(&x.to_ascii_lowercase().as_str()));
                if ok {
                    out.push(p);
                }
            }
            _ => {}
        }
    }
}

/// Identifiant court et stable dérivé du chemin.
fn id_of(path: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.to_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("{h:016x}")
}

fn read_track(path: &Path, mtime: u64) -> Option<LocalTrack> {
    let tagged = lofty::read_from_path(path).ok()?;
    let props = tagged.properties();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let get = |f: &dyn Fn(&lofty::tag::Tag) -> Option<String>| tag.and_then(f).unwrap_or_default();
    let p = path.to_string_lossy().to_string();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let format = match tagged.file_type() {
        lofty::file::FileType::Flac => "FLAC".to_string(),
        lofty::file::FileType::Mpeg => "MP3".to_string(),
        lofty::file::FileType::Wav => "WAV".to_string(),
        lofty::file::FileType::Aiff => "AIFF".to_string(),
        lofty::file::FileType::Vorbis => "Vorbis".to_string(),
        lofty::file::FileType::Mp4 => {
            // ALAC ou AAC selon le débit : un ALAC dépasse largement 400 kb/s
            if props.audio_bitrate().unwrap_or(0) > 400 { "ALAC".into() } else { "AAC".into() }
        }
        _ => ext.to_uppercase(),
    };
    let lossless = matches!(format.as_str(), "FLAC" | "ALAC" | "WAV" | "AIFF" | "CAF");
    let gain = tag
        .and_then(|t| t.get_string(&lofty::tag::ItemKey::ReplayGainTrackGain))
        .and_then(|s| s.trim().trim_end_matches("dB").trim().replace(',', ".").parse::<f32>().ok())
        .unwrap_or(0.0);
    let artist = get(&|t| t.artist().map(|s| s.to_string()));
    Some(LocalTrack {
        id: id_of(&p),
        title: Some(get(&|t| t.title().map(|s| s.to_string()))).filter(|s| !s.is_empty()).unwrap_or(stem),
        album_artist: Some(get(&|t| t.get_string(&lofty::tag::ItemKey::AlbumArtist).map(|s| s.to_string())))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| artist.clone()),
        artist,
        // fichier sans étiquette d'album : le nom de son dossier (sinon tout finit dans « Sans album »)
        album: Some(get(&|t| t.album().map(|s| s.to_string())))
            .filter(|s| !s.is_empty())
            .or_else(|| path.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_default(),
        genre: get(&|t| t.genre().map(|s| s.to_string())),
        year: tag.and_then(|t| t.year()).unwrap_or(0),
        track: tag.and_then(|t| t.track()).unwrap_or(0),
        disc: tag.and_then(|t| t.disk()).unwrap_or(0),
        duration: props.duration().as_secs_f64(),
        rate: props.sample_rate().unwrap_or(0),
        bits: props.bit_depth().unwrap_or(0),
        bitrate: props.audio_bitrate().unwrap_or(0),
        format,
        lossless,
        gain,
        art: tag.is_some_and(|t| !t.pictures().is_empty()) || folder_cover(path).is_some(),
        mtime,
        path: p,
    })
}

/// Lecture d'un fichier isolé (glissé dans la fenêtre).
pub fn read_file(path: &Path) -> Option<LocalTrack> {
    let ok = path
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| EXTENSIONS.contains(&x.to_ascii_lowercase().as_str()));
    if !ok {
        return None;
    }
    std::panic::catch_unwind(|| read_track(path, 0)).ok().flatten()
}

/// Pochette intégrée (ou fichier cover/folder.jpg du dossier).
pub fn artwork(path: &str) -> Option<(Vec<u8>, &'static str)> {
    if let Ok(tagged) = lofty::read_from_path(path) {
        if let Some(pic) = tagged.primary_tag().or_else(|| tagged.first_tag()).and_then(|t| t.pictures().first().cloned()) {
            let mime = match pic.mime_type() {
                Some(lofty::picture::MimeType::Png) => "image/png",
                _ => "image/jpeg",
            };
            return Some((pic.data().to_vec(), mime));
        }
    }
    let cover = folder_cover(Path::new(path))?;
    let png = cover.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
    std::fs::read(&cover).ok().map(|b| (b, if png { "image/png" } else { "image/jpeg" }))
}

/// Image de pochette posée dans le dossier du morceau (noms usuels ; Windows
/// ne distingue pas les majuscules).
fn folder_cover(track: &Path) -> Option<PathBuf> {
    let dir = track.parent()?;
    ["cover.jpg", "cover.jpeg", "cover.png", "folder.jpg", "folder.png", "front.jpg", "front.png", "albumart.jpg"]
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

/// Paroles d'un fichier local : .lrc à côté du morceau, sinon étiquette.
pub fn local_lyrics(path: &str) -> Option<String> {
    let lrc = Path::new(path).with_extension("lrc");
    if let Ok(s) = std::fs::read_to_string(&lrc) {
        return Some(s);
    }
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    tag.get_string(&lofty::tag::ItemKey::Lyrics).map(|s| s.to_string())
}
