<p align="center"><img src="captures/icone.png" width="112" alt=""></p>

<h1 align="center">Pomme</h1>

<p align="center"><b>Un lettore musicale leggero per Apple Music e i tuoi file audio: testi, equalizzatore, mini player.</b></p>

<p align="center"><img src="https://img.shields.io/badge/%F0%9F%8E%AE%20Music%20%26%20games-Windows-c2410c?style=flat-square" alt="Musica e giochi"> <a href="https://github.com/jeremdevgit/pomme/releases"><img src="https://img.shields.io/github/v/release/jeremdevgit/pomme?include_prereleases&style=flat-square&label=version" alt="version"></a> <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--or--later-555?style=flat-square" alt="GPL-3.0-or-later"></a> <a href="https://github.com/jeremdevgit/pomme/actions/workflows/windows.yml"><img src="https://github.com/jeremdevgit/pomme/actions/workflows/windows.yml/badge.svg" alt="CI"></a></p>

<p align="center"><a href="README.md">English</a> · <a href="README.fr.md">Français</a> · <a href="README.es.md">Español</a> · <a href="README.de.md">Deutsch</a> · <b>Italiano</b> · <a href="README.pt.md">Português</a> · <a href="README.zh.md">简体中文</a> · <a href="README.ja.md">日本語</a></p>

<p align="center"><a href="https://github.com/jeremdevgit/pomme/releases/latest"><b>⬇ Scarica</b></a></p>

![Album locali e il lettore](captures/en/albums.jpg)

Apple Music e i tuoi file audio nella stessa finestra, in un'app di circa dieci megabyte — **senza Electron**.

## In breve

| | |
|---|---|
| **Categoria** | 🎮 Musica e giochi |
| **Piattaforme** | Windows |
| **Interfaccia** | 8 lingue (EN, FR, ES, DE, IT, PT, ZH, JA) |
| **Prezzo** | Gratuito, open source, senza pubblicità né telemetria |
| **Stato** | Stabile |
| **Licenza** | [GPL-3.0-or-later](LICENSE) |

## Funzioni

- **Apple Music** (abbonamento richiesto): Ascolta ora, Esplora, Radio, la tua libreria e playlist, ricerca, creazione di playlist.
- **I tuoi file**: FLAC, ALAC, MP3, AAC/M4A, Ogg Vorbis, WAV, AIFF, CAF, MKA… con copertine, album e artisti; trascina e rilascia; riproduzione ad alta risoluzione.
- **Testi sincronizzati** a schermo intero (Apple Music, o LRCLIB per i brani locali — disattivabile).
- **Equalizzatore** a 10 bande con preset, normalizzazione del volume, scelta dell'uscita audio, qualità di riproduzione.
- **Mini player**, timer di spegnimento, tasti multimediali e controlli di Windows, area di notifica, avvio automatico.
- **Stato Discord** e **streaming di rete** verso un ricevitore compatibile sulla stessa rete (Windows 11), se vuoi.

<p><img src="captures/en/album.jpg" alt="Un album" width="49%"> <img src="captures/en/reglages.jpg" alt="Impostazioni" width="49%"></p>

## Per iniziare

1. Scarica `Pomme-Setup.exe` dalla [versione più recente](https://github.com/jeremdevgit/pomme/releases/latest) ed eseguilo. Pomme si installa **per il tuo account, senza diritti di amministratore**, e sostituisce una versione precedente mantenendo libreria, impostazioni e accesso ad Apple Music.
2. Accedi ad Apple Music sulla pagina ufficiale di Apple, oppure trascina le cartelle di musica nella finestra.
3. Disinstalla da *Impostazioni › App*, conservando o cancellando la libreria.

Requisiti: Windows 10 o 11 con WebView2 (incluso in Windows 11). Nessun certificato a pagamento: se compare SmartScreen, scegli **Ulteriori informazioni › Esegui comunque**. Verifica con `Get-FileHash` e `SHA256SUMS.txt`. Installazione silenziosa: `Pomme-Setup.exe /S`.

## Privacy

- **Niente telemetria, niente account Pomme.** La sessione di Apple Music resta nel profilo di Pomme sul tuo PC.
- Servizi contattati: Apple Music; LRCLIB per i testi locali (solo titolo e artista, disattivabile); Discord in locale se lo stato è attivo. Lo streaming di rete parte solo dopo aver scelto un dispositivo.
- Pomme non include alcuna chiave Apple: come il player web di Apple, usa il token pubblico pubblicato da music.apple.com.

## Compilare

```powershell
# Rust stable + Visual Studio Build Tools (C++)
cargo build --release        # target\release\Pomme.exe
.\tools\release.ps1          # dist\Pomme-Setup.exe + controlli
```

Le traduzioni sono in `ui/textes/<lingua>.js` (testo francese come chiave). Le versioni sono compilate da GitHub Actions ([`windows.yml`](.github/workflows/windows.yml)).

## Licenza

GNU GPL v3 o successiva — vedi [LICENSE](LICENSE). Pomme non è affiliato né approvato da Apple; Apple Music è un marchio di Apple Inc. Le librerie Rust (symphonia, cpal, wry, tao…) sono MIT / Apache-2.0 / MPL-2.0.

---

Hai trovato una vulnerabilità? Segnalala in privato (vedi [SECURITY.md](SECURITY.md)) e non in una issue pubblica.

## Altro software libero di jeremdevgit

Tutti gratuiti, open source e senza pubblicità.

**🛠️ Sistema e hardware** — [Boîte à outils](https://github.com/jeremdevgit/boite-a-outils) · [rJb](https://github.com/jeremdevgit/rjb) · [LJ Hub](https://github.com/jeremdevgit/lj-hub) · [Decklight](https://github.com/jeremdevgit/decklight)

**📒 Produttività e dati** — [Carnet de notes](https://github.com/jeremdevgit/carnet-rust) · [JMeca](https://github.com/jeremdevgit/jmeca) · [Suite Libre](https://jeremdevgit.github.io/suite-libre/)

**🎮 Musica e giochi** — [Ludothèque](https://github.com/jeremdevgit/ludotheque) · [AutoDex](https://github.com/jeremdevgit/autodex) · [Batailles de France](https://github.com/jeremdevgit/batailles-de-france)
