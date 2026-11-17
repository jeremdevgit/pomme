<p align="center"><img src="captures/icone.png" width="112" alt=""></p>

<h1 align="center">Pomme</h1>

<p align="center"><b>Um leitor de música leve para Apple Music e os seus ficheiros de áudio: letras, equalizador, minileitor.</b></p>

<p align="center"><img src="https://img.shields.io/badge/%F0%9F%8E%AE%20Music%20%26%20games-Windows-c2410c?style=flat-square" alt="Música e jogos"> <a href="https://github.com/jeremdevgit/pomme/releases"><img src="https://img.shields.io/github/v/release/jeremdevgit/pomme?include_prereleases&style=flat-square&label=version" alt="version"></a> <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--or--later-555?style=flat-square" alt="GPL-3.0-or-later"></a> <a href="https://github.com/jeremdevgit/pomme/actions/workflows/windows.yml"><img src="https://github.com/jeremdevgit/pomme/actions/workflows/windows.yml/badge.svg" alt="CI"></a></p>

<p align="center"><a href="README.md">English</a> · <a href="README.fr.md">Français</a> · <a href="README.es.md">Español</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <b>Português</b> · <a href="README.zh.md">简体中文</a> · <a href="README.ja.md">日本語</a></p>

<p align="center"><a href="https://github.com/jeremdevgit/pomme/releases/latest"><b>⬇ Transferir</b></a></p>

![Álbuns locais e o leitor](captures/en/albums.jpg)

Apple Music e os seus próprios ficheiros de áudio na mesma janela, numa app de cerca de dez megabytes — **sem Electron**.

## Em resumo

| | |
|---|---|
| **Categoria** | 🎮 Música e jogos |
| **Plataformas** | Windows |
| **Interface** | 8 idiomas (EN, FR, ES, DE, IT, PT, ZH, JA) |
| **Preço** | Gratuito, de código aberto, sem anúncios nem telemetria |
| **Estado** | Estável |
| **Licença** | [GPL-3.0-or-later](LICENSE) |

## Funções

- **Apple Music** (requer subscrição): Ouvir agora, Explorar, Rádio, a sua biblioteca e playlists, pesquisa, criação de playlists.
- **Os seus ficheiros**: FLAC, ALAC, MP3, AAC/M4A, Ogg Vorbis, WAV, AIFF, CAF, MKA… com capas, álbuns e artistas; arrastar e largar; reprodução em alta resolução.
- **Letras sincronizadas** em ecrã inteiro (Apple Music, ou LRCLIB para músicas locais — pode desativar).
- **Equalizador** de 10 bandas com predefinições, normalização do volume, escolha da saída de áudio, qualidade de reprodução.
- **Minileitor**, temporizador, teclas multimédia e controlos do Windows, área de notificação, arranque automático.
- **Estado no Discord** e **transmissão em rede** para um recetor compatível na mesma rede (Windows 11), se quiser.

<p><img src="captures/en/album.jpg" alt="Um álbum" width="49%"> <img src="captures/en/reglages.jpg" alt="Definições" width="49%"></p>

## Começar

1. Transfira `Pomme-Setup.exe` da [versão mais recente](https://github.com/jeremdevgit/pomme/releases/latest) e execute-o. O Pomme instala-se **para a sua conta, sem direitos de administrador**, e substitui uma versão anterior mantendo biblioteca, definições e sessão do Apple Music.
2. Inicie sessão no Apple Music na página oficial da Apple, ou largue as suas pastas de música na janela.
3. Desinstale em *Definições › Aplicações*, mantendo ou apagando a biblioteca.

Requisitos: Windows 10 ou 11 com WebView2 (incluído no Windows 11). Sem certificado pago: se o SmartScreen aparecer, escolha **Mais informações › Executar mesmo assim**. Verifique com `Get-FileHash` e `SHA256SUMS.txt`. Instalação silenciosa: `Pomme-Setup.exe /S`.

## Privacidade

- **Sem telemetria, sem conta Pomme.** A sua sessão do Apple Music fica no perfil do Pomme no seu PC.
- Serviços contactados: Apple Music; LRCLIB para letras locais (apenas título e artista, pode desativar); Discord localmente se o estado estiver ativo. A transmissão em rede só começa depois de escolher um dispositivo.
- O Pomme não inclui nenhuma chave da Apple: tal como o leitor web da Apple, usa o token público publicado por music.apple.com.

## Compilar

```powershell
# Rust estável + Visual Studio Build Tools (C++)
cargo build --release        # target\release\Pomme.exe
.\tools\release.ps1          # dist\Pomme-Setup.exe + verificações
```

As traduções estão em `ui/textes/<idioma>.js` (texto francês como chave). As versões são compiladas pelo GitHub Actions ([`windows.yml`](.github/workflows/windows.yml)).

## Licença

GNU GPL v3 ou posterior — ver [LICENSE](LICENSE). O Pomme não é afiliado nem aprovado pela Apple; Apple Music é uma marca da Apple Inc. As bibliotecas Rust (symphonia, cpal, wry, tao…) são MIT / Apache-2.0 / MPL-2.0.

---

Encontrou uma vulnerabilidade? Comunique-a em privado (consulte [SECURITY.md](SECURITY.md)) e não num issue público.

## Mais software livre de jeremdevgit

Todos gratuitos, de código aberto e sem anúncios.

**🛠️ Sistema e hardware** — [Boîte à outils](https://github.com/jeremdevgit/boite-a-outils) · [rJb](https://github.com/jeremdevgit/rjb) · [LJ Hub](https://github.com/jeremdevgit/lj-hub) · [Decklight](https://github.com/jeremdevgit/decklight)

**📒 Produtividade e dados** — [Carnet de notes](https://github.com/jeremdevgit/carnet-rust) · [JMeca](https://github.com/jeremdevgit/jmeca) · [Suite Libre](https://jeremdevgit.github.io/suite-libre/)

**🎮 Música e jogos** — [Ludothèque](https://github.com/jeremdevgit/ludotheque) · [AutoDex](https://github.com/jeremdevgit/autodex) · [Batailles de France](https://github.com/jeremdevgit/batailles-de-france)
