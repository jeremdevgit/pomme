<p align="center"><img src="captures/icone.png" width="112" alt=""></p>

<h1 align="center">Pomme</h1>

<p align="center"><b>Un reproductor ligero para Apple Music y tus archivos de audio: letras, ecualizador, minirreproductor.</b></p>

<p align="center"><img src="https://img.shields.io/badge/%F0%9F%8E%AE%20Music%20%26%20games-Windows-c2410c?style=flat-square" alt="Música y juegos"> <a href="https://github.com/jeremdevgit/pomme/releases"><img src="https://img.shields.io/github/v/release/jeremdevgit/pomme?include_prereleases&style=flat-square&label=version" alt="version"></a> <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--or--later-555?style=flat-square" alt="GPL-3.0-or-later"></a> <a href="https://github.com/jeremdevgit/pomme/actions/workflows/windows.yml"><img src="https://github.com/jeremdevgit/pomme/actions/workflows/windows.yml/badge.svg" alt="CI"></a></p>

<p align="center"><a href="README.md">English</a> · <a href="README.fr.md">Français</a> · <b>Español</b> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pt.md">Português</a> · <a href="README.zh.md">简体中文</a> · <a href="README.ja.md">日本語</a></p>

<p align="center"><a href="https://github.com/jeremdevgit/pomme/releases/latest"><b>⬇ Descargar</b></a></p>

![Álbumes locales y el reproductor](captures/en/albums.jpg)

Apple Music y tus propios archivos de audio en la misma ventana, en una app de unos diez megabytes, **sin Electron**.

## De un vistazo

| | |
|---|---|
| **Categoría** | 🎮 Música y juegos |
| **Plataformas** | Windows |
| **Interfaz** | 8 idiomas (EN, FR, ES, DE, IT, PT, ZH, JA) |
| **Precio** | Gratis, de código abierto, sin anuncios ni telemetría |
| **Estado** | Estable |
| **Licencia** | [GPL-3.0-or-later](LICENSE) |

## Funciones

- **Apple Music** (requiere suscripción): Escuchar, Explorar, Radio, tu biblioteca y listas, búsqueda, creación de listas.
- **Tus archivos**: FLAC, ALAC, MP3, AAC/M4A, Ogg Vorbis, WAV, AIFF, CAF, MKA… con carátulas, álbumes y artistas; arrastrar y soltar; reproducción en alta resolución.
- **Letras sincronizadas** a pantalla completa (Apple Music, o LRCLIB para canciones locales; se puede desactivar).
- **Ecualizador** de 10 bandas con preajustes, normalización de volumen, elección de salida de audio, calidad de reproducción.
- **Minirreproductor**, temporizador, teclas multimedia y controles de Windows, área de notificación, inicio con Windows.
- **Estado de Discord** y **transmisión por red** a un receptor compatible de la misma red (Windows 11), si quieres.

<p><img src="captures/en/album.jpg" alt="Un álbum" width="49%"> <img src="captures/en/reglages.jpg" alt="Ajustes" width="49%"></p>

## Para empezar

1. Descarga `Pomme-Setup.exe` de la [última versión](https://github.com/jeremdevgit/pomme/releases/latest) y ejecútalo. Pomme se instala **para tu cuenta, sin permisos de administrador**, y reemplaza una versión anterior conservando biblioteca, ajustes y sesión de Apple Music.
2. Inicia sesión en Apple Music en la página oficial de Apple, o suelta tus carpetas de música en la ventana.
3. Desinstala desde *Configuración › Aplicaciones*, conservando o borrando tu biblioteca.

Requisitos: Windows 10 u 11 con WebView2 (incluido en Windows 11). Sin certificado de pago: si aparece SmartScreen, elige **Más información › Ejecutar de todas formas**. Comprueba con `Get-FileHash` y `SHA256SUMS.txt`. Instalación silenciosa: `Pomme-Setup.exe /S`.

## Privacidad

- **Sin telemetría ni cuenta de Pomme.** Tu sesión de Apple Music se queda en el perfil de Pomme en tu PC.
- Servicios contactados: Apple Music; LRCLIB para letras locales (solo título y artista, desactivable); Discord en local si el estado está activado. La transmisión por red solo empieza tras elegir un dispositivo.
- Pomme no incluye ninguna clave de Apple: como el reproductor web de Apple, usa el token público publicado por music.apple.com.

## Compilar

```powershell
# Rust estable + Visual Studio Build Tools (C++)
cargo build --release        # target\release\Pomme.exe
.\tools\release.ps1          # dist\Pomme-Setup.exe + comprobaciones
```

Las traducciones están en `ui/textes/<idioma>.js` (texto francés como clave). Las versiones se compilan con GitHub Actions ([`windows.yml`](.github/workflows/windows.yml)).

## Licencia

GNU GPL v3 o posterior — ver [LICENSE](LICENSE). Pomme no está afiliado ni respaldado por Apple; Apple Music es una marca de Apple Inc. Las bibliotecas Rust (symphonia, cpal, wry, tao…) son MIT / Apache-2.0 / MPL-2.0.

---

¿Has encontrado una vulnerabilidad? Infórmala en privado (consulta [SECURITY.md](SECURITY.md)) y no en una incidencia pública.

## Más software libre de jeremdevgit

Todos gratuitos, de código abierto y sin anuncios.

**🛠️ Sistema y hardware** — [Boîte à outils](https://github.com/jeremdevgit/boite-a-outils) · [rJb](https://github.com/jeremdevgit/rjb) · [LJ Hub](https://github.com/jeremdevgit/lj-hub) · [Decklight](https://github.com/jeremdevgit/decklight)

**📒 Productividad y datos** — [Carnet de notes](https://github.com/jeremdevgit/carnet-rust) · [JMeca](https://github.com/jeremdevgit/jmeca) · [Suite Libre](https://jeremdevgit.github.io/suite-libre/)

**🎮 Música y juegos** — [Ludothèque](https://github.com/jeremdevgit/ludotheque) · [AutoDex](https://github.com/jeremdevgit/autodex) · [Batailles de France](https://github.com/jeremdevgit/batailles-de-france)
