// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Réglages natifs (fenêtre, démarrage, accélération graphique). Les réglages
//! de l'interface (volume, égaliseur…) sont gardés par l'interface elle-même.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// rendu par la carte graphique (plus de mémoire, défilement plus doux sur les grands écrans)
    pub gpu: bool,
    /// fermer la fenêtre garde Pomme dans la zone de notification
    pub close_to_tray: bool,
    pub autostart: bool,
    /// statut « Écoute » sur Discord
    pub discord: bool,
    /// identifiant de l'application Discord qui porte le nom et l'icône affichés
    pub discord_id: String,
}

/// Aucune application Discord par défaut : le statut demande un identifiant choisi par l'utilisateur.
pub const DISCORD_ID: &str = "";

impl Default for Config {
    fn default() -> Self {
        Config {
            gpu: true,
            close_to_tray: true,
            autostart: false,
            discord: false,
            discord_id: DISCORD_ID.into(),
        }
    }
}

pub fn dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(if std::env::var_os("POMME_TEST").is_some() { "PommeTest" } else { "Pomme" })
}

impl Config {
    fn path() -> PathBuf {
        dir().join("config.json")
    }

    pub fn load() -> Config {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
            .map(|mut c| {
                // identifiant vide : application Discord de Pomme
                if c.discord_id.trim().is_empty() {
                    c.discord_id = DISCORD_ID.into();
                }
                c
            })
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(dir());
        if let Ok(j) = serde_json::to_vec_pretty(self) {
            let p = Self::path();
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, j).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

/// Lancement avec Windows, réduit dans la zone de notification.
pub fn set_autostart(on: bool) {
    use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;
    match (on, std::env::current_exe()) {
        (true, Ok(exe)) => {
            crate::sys::reg_set_str(HKEY_CURRENT_USER, RUN_KEY, "Pomme", &format!("\"{}\" --minimized", exe.display()));
        }
        _ => crate::sys::reg_delete_value(HKEY_CURRENT_USER, RUN_KEY, "Pomme"),
    }
}
