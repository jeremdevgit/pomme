<p align="center"><img src="captures/icone.png" width="112" alt=""></p>

<h1 align="center">Pomme</h1>

<p align="center"><b>A lightweight music player for Apple Music and your local audio files: lyrics, equalizer, mini player.</b></p>

<p align="center"><img src="https://img.shields.io/badge/%F0%9F%8E%AE%20Music%20%26%20games-Windows-c2410c?style=flat-square" alt="Music & games"> <a href="https://github.com/jeremdevgit/pomme/releases"><img src="https://img.shields.io/github/v/release/jeremdevgit/pomme?include_prereleases&style=flat-square&label=version" alt="version"></a> <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--or--later-555?style=flat-square" alt="GPL-3.0-or-later"></a> <a href="https://github.com/jeremdevgit/pomme/actions/workflows/windows.yml"><img src="https://github.com/jeremdevgit/pomme/actions/workflows/windows.yml/badge.svg" alt="CI"></a></p>

<p align="center"><b>English</b> · <a href="README.fr.md">Français</a> · <a href="README.es.md">Español</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pt.md">Português</a> · <a href="README.zh.md">简体中文</a> · <a href="README.ja.md">日本語</a></p>

<p align="center"><a href="https://github.com/jeremdevgit/pomme/releases/latest"><b>⬇ Download</b></a></p>

![Local albums and the player](captures/en/albums.jpg)

Apple Music and your own audio files in the same window, in an app of about ten megabytes — **no Electron**.

## At a glance

| | |
|---|---|
| **Category** | 🎮 Music & games |
| **Platforms** | Windows |
| **Interface** | 8 languages (EN, FR, ES, DE, IT, PT, ZH, JA) |
| **Price** | Free, open source, no ads, no telemetry |
| **Status** | Stable |
| **License** | [GPL-3.0-or-later](LICENSE) |

## Features

- **Apple Music** (subscription required): Listen Now, Browse, Radio, your library and playlists, search, playlist creation.
- **Your music files**: FLAC, ALAC, MP3, AAC/M4A, Ogg Vorbis, WAV, AIFF, CAF, MKA… with artwork, albums and artists; drag and drop; high-resolution playback.
- Full-screen **synced lyrics** (Apple Music, or LRCLIB for local songs — can be turned off).
- 10-band **equalizer** with presets, volume normalization, audio output choice, playback quality.
- **Mini player**, sleep timer, media keys and Windows controls, notification area, launch at startup.
- Optional **Discord status** and **network streaming** to a compatible receiver on the same network (Windows 11).

<p><img src="captures/en/album.jpg" alt="An album" width="49%"> <img src="captures/en/reglages.jpg" alt="Settings" width="49%"></p>

## Get started

1. Download `Pomme-Setup.exe` from the [latest release](https://github.com/jeremdevgit/pomme/releases/latest) and run it. Pomme installs **for your account, without administrator rights**, and replaces an older version while keeping your library, settings and Apple Music sign-in.
2. Sign in to Apple Music on Apple's official page, or drop your music folders into the window.
3. Uninstall from *Settings › Apps*, keeping or erasing your library.

Requirements: Windows 10 or 11 with WebView2 (built into Windows 11). Not signed with a paid certificate: if SmartScreen appears, choose **More info › Run anyway**. Verify with `Get-FileHash` against `SHA256SUMS.txt`. Silent install: `Pomme-Setup.exe /S`.

## Privacy

- **No telemetry, no Pomme account.** Your Apple Music session stays in Pomme's profile on your PC.
- Services contacted: Apple Music; LRCLIB for local lyrics (title and artist only, can be turned off); Discord locally if the status is on. Network streaming starts only after you pick a device.
- Pomme ships no Apple key: like Apple's web player, it uses the public token published by music.apple.com.

## Build from source

```powershell
# Rust stable + Visual Studio Build Tools (C++)
cargo build --release        # target\release\Pomme.exe
.\tools\release.ps1          # dist\Pomme-Setup.exe + checks
```

Translations live in `ui/textes/<language>.js` (French text as key). Releases are built by GitHub Actions ([`windows.yml`](.github/workflows/windows.yml)).

## License

GNU GPL v3 or later — see [LICENSE](LICENSE). Pomme is not affiliated with or endorsed by Apple; Apple Music is a trademark of Apple Inc. Rust libraries (symphonia, cpal, wry, tao…) are MIT / Apache-2.0 / MPL-2.0.

---

Found a vulnerability? Please report it privately (see [SECURITY.md](SECURITY.md)) rather than in a public issue.

## More free software by jeremdevgit

All free, open source and ad-free.

**🛠️ System & hardware** — [Boîte à outils](https://github.com/jeremdevgit/boite-a-outils) · [rJb](https://github.com/jeremdevgit/rjb) · [LJ Hub](https://github.com/jeremdevgit/lj-hub) · [Decklight](https://github.com/jeremdevgit/decklight)

**📒 Productivity & data** — [Carnet de notes](https://github.com/jeremdevgit/carnet-rust) · [JMeca](https://github.com/jeremdevgit/jmeca) · [Suite Libre](https://jeremdevgit.github.io/suite-libre/)

**🎮 Music & games** — [Ludothèque](https://github.com/jeremdevgit/ludotheque) · [AutoDex](https://github.com/jeremdevgit/autodex) · [Batailles de France](https://github.com/jeremdevgit/batailles-de-france)
