// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Langue des quelques textes affichés par Windows lui-même (menu de la zone de notification,
//! boîte de choix de dossier, titre de l'installateur, raccourcis, Paramètres › Applications) :
//! celle choisie dans l'interface, transmise au démarrage. Anglais par défaut.
use std::sync::atomic::{AtomicUsize, Ordering};

const CODES: [&str; 8] = ["fr", "en", "es", "de", "it", "pt", "zh", "ja"];
static LANGUE: AtomicUsize = AtomicUsize::new(1);

/// Change la langue ; renvoie vrai si elle a changé.
pub fn definir(code: &str) -> bool {
    match CODES.iter().position(|c| *c == code) {
        Some(i) => LANGUE.swap(i, Ordering::Relaxed) != i,
        None => false,
    }
}

/// Texte dans la langue choisie (le français sert de clé).
pub fn tr(fr: &'static str) -> &'static str {
    let i = LANGUE.load(Ordering::Relaxed);
    if i == 0 {
        return fr;
    }
    TEXTES.iter().find(|t| t[0] == fr).map_or(fr, |t| t[i])
}

#[rustfmt::skip]
const TEXTES: &[[&str; 8]] = &[
    ["Ouvrir Pomme", "Open Pomme", "Abrir Pomme", "Pomme öffnen", "Apri Pomme", "Abrir o Pomme", "打开 Pomme", "Pomme を開く"],
    ["Lecture / Pause", "Play / Pause", "Reproducir / Pausa", "Wiedergabe / Pause", "Riproduci / Pausa", "Reproduzir / Pausar", "播放 / 暂停", "再生 / 一時停止"],
    ["Suivant", "Next", "Siguiente", "Weiter", "Successivo", "Próxima", "下一首", "次へ"],
    ["Précédent", "Previous", "Anterior", "Zurück", "Precedente", "Anterior", "上一首", "前へ"],
    ["Quitter", "Quit", "Salir", "Beenden", "Esci", "Sair", "退出", "終了"],
    ["Choisir un dossier de musique", "Choose a music folder", "Elegir una carpeta de música", "Musikordner auswählen", "Scegli una cartella di musica", "Escolher uma pasta de música", "选择音乐文件夹", "音楽フォルダーを選択"],
    ["Choisir le dossier d'installation", "Choose the installation folder", "Elegir la carpeta de instalación", "Installationsordner auswählen", "Scegli la cartella di installazione", "Escolher a pasta de instalação", "选择安装文件夹", "インストール先フォルダーを選択"],
    ["Installer Pomme", "Install Pomme", "Instalar Pomme", "Pomme installieren", "Installa Pomme", "Instalar o Pomme", "安装 Pomme", "Pomme をインストール"],
    ["Pomme — désinstaller ou réparer", "Pomme — uninstall or repair", "Pomme: desinstalar o reparar", "Pomme – deinstallieren oder reparieren", "Pomme — disinstalla o ripara", "Pomme — desinstalar ou reparar", "Pomme — 卸载或修复", "Pomme — アンインストールまたは修復"],
    ["Lecteur Apple Music et musique locale", "Apple Music and local music player", "Reproductor de Apple Music y música local", "Player für Apple Music und lokale Musik", "Lettore Apple Music e musica locale", "Player de Apple Music e música local", "Apple Music 与本地音乐播放器", "Apple Music とローカル音楽のプレーヤー"],
    ["Lecteur Apple Music et musique locale, léger et fluide", "Lightweight, smooth player for Apple Music and local music", "Reproductor ligero y fluido de Apple Music y música local", "Leichter, flüssiger Player für Apple Music und lokale Musik", "Lettore Apple Music e musica locale, leggero e fluido", "Player leve e fluido de Apple Music e música local", "轻巧流畅的 Apple Music 与本地音乐播放器", "軽快でなめらかな Apple Music とローカル音楽のプレーヤー"],
    ["Artiste inconnu", "Unknown artist", "Artista desconocido", "Unbekannter Künstler", "Artista sconosciuto", "Artista desconhecido", "未知艺术家", "不明なアーティスト"],
];
