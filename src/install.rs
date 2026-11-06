// SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Installation, réparation et désinstallation pour l'utilisateur courant :
//! aucun droit administrateur, aucune invite UAC. Rien de personnel n'est
//! écrit ailleurs que dans le profil Windows de la personne qui installe.

use crate::sys;
use std::path::{Path, PathBuf};
use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;
use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Programs};

pub const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Pomme";
pub const APP_KEY: &str = "Software\\Pomme";
pub const APP_EXE: &str = "Pomme.exe";
pub const MAINTENANCE_EXE: &str = "uninstall.exe";

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Options {
    pub dir: String,
    pub desktop: bool,
    pub start_menu: bool,
    pub autostart: bool,
    pub close_to_tray: bool,
}

impl Default for Options {
    fn default() -> Self {
        let cfg = crate::config::Config::load();
        let get = |n: &str, d: bool| sys::reg_get_dword(HKEY_CURRENT_USER, APP_KEY, n).map(|v| v != 0).unwrap_or(d);
        Options {
            dir: installed_dir().unwrap_or_else(default_dir).to_string_lossy().into_owned(),
            desktop: get("Desktop", true),
            start_menu: get("StartMenu", true),
            autostart: cfg.autostart,
            close_to_tray: cfg.close_to_tray,
        }
    }
}

pub fn default_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\"))
        .join("Programs")
        .join("Pomme")
}

/// Version déjà installée (affichée par l'installateur, qui la remplace).
pub fn installed_version() -> Option<String> {
    sys::reg_get_str(HKEY_CURRENT_USER, UNINSTALL_KEY, "DisplayVersion").filter(|s| !s.is_empty())
}

pub fn installed_dir() -> Option<PathBuf> {
    sys::reg_get_str(HKEY_CURRENT_USER, UNINSTALL_KEY, "InstallLocation")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn eq_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches('\\'))
}

/// Vérifie le dossier choisi ; un dossier existant qui contient autre chose
/// reçoit un sous-dossier « Pomme ».
pub fn normalize_dir(input: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(input.trim().trim_matches('"'));
    let s = p.to_string_lossy().to_string();
    if !(s.len() > 3 && s.as_bytes()[1] == b':' && s.as_bytes()[2] == b'\\') {
        return Err("Indiquez un chemin complet, par exemple C:\\Users\\vous\\AppData\\Local\\Programs\\Pomme".into());
    }
    if s.contains(['<', '>', '"', '|', '?', '*']) {
        return Err("Le chemin contient des caractères interdits.".into());
    }
    let windir = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    if p.to_string_lossy().to_lowercase().starts_with(&windir.to_string_lossy().to_lowercase()) {
        return Err("Impossible d'installer dans le dossier de Windows.".into());
    }
    let non_empty = std::fs::read_dir(&p).is_ok_and(|mut it| it.next().is_some());
    Ok(if non_empty && !p.join(APP_EXE).exists() { p.join("Pomme") } else { p })
}

/// Demande à Pomme de se fermer et attend son départ.
pub fn stop_running_app() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenEventW, OpenMutexW, SetEvent, EVENT_MODIFY_STATE, SYNCHRONIZATION_SYNCHRONIZE};
    let ev = sys::w("Local\\Pomme.quit");
    let mx = sys::w("Local\\Pomme.instance");
    unsafe {
        let e = OpenEventW(EVENT_MODIFY_STATE, 0, ev.as_ptr());
        if !e.is_null() {
            SetEvent(e);
            CloseHandle(e);
        }
        for _ in 0..100 {
            let m = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, mx.as_ptr());
            if m.is_null() {
                // les processus du moteur web se ferment juste après
                std::thread::sleep(std::time::Duration::from_millis(800));
                return true;
            }
            CloseHandle(m);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    false
}

fn shortcut_paths() -> (Option<PathBuf>, Option<PathBuf>) {
    (
        sys::known_folder(&FOLDERID_Desktop).map(|d| d.join("Pomme.lnk")),
        sys::known_folder(&FOLDERID_Programs).map(|d| d.join("Pomme.lnk")),
    )
}

fn copy_atomic(src: &Path, dst: &Path) -> std::io::Result<()> {
    if eq_path(src, dst) {
        return Ok(());
    }
    let tmp = dst.with_extension("new");
    std::fs::copy(src, &tmp)?;
    std::fs::rename(&tmp, dst).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn dir_size_kb(dir: &Path) -> u32 {
    std::fs::read_dir(dir)
        .map(|it| it.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum::<u64>())
        .unwrap_or(0) as u32
        / 1024
}

pub fn install(o: &Options, step: &dyn Fn(&str)) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    let dir = normalize_dir(&o.dir)?;
    let src = std::env::current_exe().map_err(|e| e.to_string())?;
    let previous = installed_dir();
    step("Fermeture de Pomme s'il est ouvert…");
    if !stop_running_app() {
        return Err("Pomme est toujours ouvert. Quittez-le depuis la zone de notification puis réessayez.".into());
    }
    step("Copie des fichiers…");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Impossible de créer le dossier : {e}"))?;
    let app = dir.join(APP_EXE);
    copy_atomic(&src, &app).map_err(|e| format!("Copie de Pomme.exe impossible : {e}"))?;
    copy_atomic(&src, &dir.join(MAINTENANCE_EXE)).map_err(|e| format!("Copie du programme de désinstallation impossible : {e}"))?;
    // ancienne version : ses fichiers sont retirés une fois la nouvelle copiée
    // (bibliothèque, réglages et connexion Apple Music sont conservés)
    if let Some(old) = previous {
        step("Retrait de l'ancienne version…");
        let moved = !eq_path(&old, &dir);
        for f in [APP_EXE, MAINTENANCE_EXE, "Pomme.new", "uninstall.new"] {
            let file = old.join(f);
            if (moved || f.ends_with(".new")) && !eq_path(&file, &src) {
                let _ = std::fs::remove_file(file);
            }
        }
        if moved {
            let _ = std::fs::remove_dir(&old); // seulement s'il est vide
        }
    }

    step("Raccourcis…");
    let (desk, menu) = shortcut_paths();
    for (want, path) in [(o.desktop, desk), (o.start_menu, menu)] {
        let Some(path) = path else { continue };
        if want {
            if !sys::create_shortcut(&path, &app, "", crate::langue::tr("Lecteur Apple Music et musique locale")) {
                notes.push(format!("Raccourci non créé : {}", path.display()));
            }
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }

    step("Inscription dans Windows…");
    let maint = dir.join(MAINTENANCE_EXE);
    let m = format!("\"{}\" --maintenance", maint.display());
    let (hk, k) = (HKEY_CURRENT_USER, UNINSTALL_KEY);
    sys::reg_delete_tree(hk, k); // inscription de l'ancienne version remplacée en entier
    sys::reg_set_str(hk, k, "DisplayName", "Pomme");
    sys::reg_set_str(hk, k, "DisplayVersion", env!("CARGO_PKG_VERSION"));
    sys::reg_set_str(hk, k, "Publisher", "jeremdevgit");
    sys::reg_set_str(hk, k, "URLInfoAbout", "https://github.com/jeremdevgit/pomme");
    sys::reg_set_str(hk, k, "URLUpdateInfo", "https://github.com/jeremdevgit/pomme/releases/latest");
    sys::reg_set_str(hk, k, "DisplayIcon", &format!("{},0", app.display()));
    sys::reg_set_str(hk, k, "InstallLocation", &dir.to_string_lossy());
    sys::reg_set_str(hk, k, "UninstallString", &m);
    sys::reg_set_str(hk, k, "ModifyPath", &m);
    sys::reg_set_str(hk, k, "QuietUninstallString", &format!("\"{}\" --uninstall", maint.display()));
    sys::reg_set_str(hk, k, "Comments", crate::langue::tr("Lecteur Apple Music et musique locale, léger et fluide"));
    sys::reg_set_dword(hk, k, "EstimatedSize", dir_size_kb(&dir));
    sys::reg_set_dword(hk, k, "NoRepair", 0);
    sys::reg_set_dword(hk, APP_KEY, "Desktop", o.desktop as u32);
    sys::reg_set_dword(hk, APP_KEY, "StartMenu", o.start_menu as u32);

    step("Préférences…");
    let mut cfg = crate::config::Config::load();
    cfg.autostart = o.autostart;
    cfg.close_to_tray = o.close_to_tray;
    cfg.save();
    // le lancement automatique vise l'exécutable installé, pas l'installateur
    let run = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    if o.autostart {
        sys::reg_set_str(hk, run, "Pomme", &format!("\"{}\" --minimized", app.display()));
    } else {
        sys::reg_delete_value(hk, run, "Pomme");
    }
    Ok(notes)
}

pub fn repair(step: &dyn Fn(&str)) -> Result<Vec<String>, String> {
    let mut o = Options::default();
    if let Some(d) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        o.dir = d.to_string_lossy().into_owned();
    }
    install(&o, step)
}

/// Désinstalle. `everything` efface aussi bibliothèque, réglages et la
/// session Apple Music enregistrée dans le moteur web.
pub fn uninstall(everything: bool, step: &dyn Fn(&str)) -> Result<(), String> {
    let dir = installed_dir()
        .or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)))
        .ok_or("Installation introuvable")?;
    step("Fermeture de Pomme…");
    if !stop_running_app() {
        return Err("Pomme est toujours ouvert. Quittez-le depuis la zone de notification puis réessayez.".into());
    }
    step("Raccourcis et démarrage automatique…");
    let (desk, menu) = shortcut_paths();
    for p in [desk, menu].into_iter().flatten() {
        let _ = std::fs::remove_file(p);
    }
    sys::reg_delete_value(HKEY_CURRENT_USER, "Software\\Microsoft\\Windows\\CurrentVersion\\Run", "Pomme");
    step("Suppression des fichiers…");
    for f in [APP_EXE, "Pomme.new", "uninstall.new"] {
        let _ = std::fs::remove_file(dir.join(f));
    }
    if everything {
        let cfg = crate::config::dir();
        for f in ["config.json", "config.tmp", "library.json", "library.tmp"] {
            let _ = std::fs::remove_file(cfg.join(f));
        }
        let _ = std::fs::remove_dir(&cfg);
        // profil du moteur web (connexion Apple, cache) : dossier propre à Pomme
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("Pomme");
            for _ in 0..10 {
                if std::fs::remove_dir_all(&p).is_ok() || !p.exists() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
    // profil temporaire du moteur web de l'installateur
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("PommeSetup"));
    step("Retrait de l'inscription Windows…");
    sys::reg_delete_tree(HKEY_CURRENT_USER, UNINSTALL_KEY);
    sys::reg_delete_tree(HKEY_CURRENT_USER, APP_KEY);
    Ok(())
}

/// Le programme de désinstallation s'efface juste après sa fermeture.
pub fn finish_uninstall() {
    use std::os::windows::process::CommandExt;
    let Ok(maint) = std::env::current_exe() else { return };
    let Some(dir) = maint.parent() else { return };
    if !maint.file_name().is_some_and(|n| n.eq_ignore_ascii_case(MAINTENANCE_EXE)) {
        return;
    }
    let script = format!("ping -n 3 127.0.0.1 >nul & del /f /q \"{}\" & rmdir \"{}\"", maint.display(), dir.display());
    let _ = std::process::Command::new(sys::system_tool("cmd.exe"))
        .raw_arg(format!("/C \"{script}\""))
        .current_dir(sys::system_tool(""))
        // CREATE_NO_WINDOW seul : avec DETACHED_PROCESS il est ignoré et `ping` ouvre une console
        .creation_flags(0x0800_0000)
        .spawn();
}

pub fn quiet_uninstall(everything: bool) -> i32 {
    let r = uninstall(everything, &|_| {});
    finish_uninstall();
    if r.is_ok() {
        0
    } else {
        1
    }
}
