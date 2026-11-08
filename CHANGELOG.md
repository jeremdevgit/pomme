# Historique des versions

## 0.4.0 — 2026-11-08 — Lecture et diffusion réseau expérimentale

- Paroles plein écran : remplissage blanc net des mots, 30 mises à jour maximum par seconde, taille réglable et mouvements réduits.
- Volume 0–100 au clavier, curseurs synchronisés dans le lecteur et en plein écran.
- Playlists : recherche par titre, artiste ou album et tri temporaire sans modifier les morceaux enregistrés.
- MusicKit : commandes de lecture mises en file, reprise après une interruption transitoire et erreur visible si elle persiste.
- Trois thèmes avec reflets statiques pour garder le défilement léger.
- Diffusion AirPlay / Chromecast intégrée, sur demande et expérimentale : découverte locale, capture limitée à Pomme, tampon borné. Requiert Windows 11 ; association AirPlay temporaire uniquement ; dépend du support WAV du Chromecast et de la capture des contenus protégés par Windows. La compatibilité réelle avec les appareils reste à confirmer.

### English

- Clearer and smoother full-screen lyrics, configurable lyric size and reduced motion.
- Numeric volume 0–100, including full screen.
- Playlist title, artist and album filters with non-destructive sorting.
- Serialized MusicKit playback and retries for transient play/pause interruptions.
- Three lightweight visual themes.
- Experimental, opt-in built-in AirPlay / Chromecast streaming on Windows 11; device compatibility and protected-content capture require hardware testing.

## 0.3.1 — 2026-11-07 — Sécurité

- Chargement des traductions à chemins constants, sorties COM contrôlées et Tokio 1.54.1. CI épinglée et publication isolée. Éditeur documenté.

### English

- Use constant translation paths, check COM outputs and update Tokio to 1.54.1; pin CI actions and isolate publication permissions. Document publisher identity.


## Maintenance CI — 2026-11-07

- Actions GitHub épinglées à des SHA ; compilation en lecture seule, publication isolée avec droits limités et étiquette passée par variable d'environnement.
- Pin GitHub Actions to commit SHAs; read-only build job and isolated publishing permissions, with release tag passed through an environment variable.
- Application 0.3.0 inchangée. / Application 0.3.0 is unchanged.

## 0.3.0 — 2026-11-06

**English**

- **8 languages**: English, French, Spanish, German, Italian, Portuguese, Chinese and Japanese. Windows' language is picked automatically; *Settings › Language* lets you switch, and the choice is remembered.
- The notification area menu and the folder picker follow the chosen language; the installer, its shortcuts and the entry in *Settings › Apps* use Windows' language.
- Error messages from the audio engine and the installer are translated, with the technical detail kept.
- Only the translations for the chosen language are loaded.

**Français**

- **8 langues** : français, anglais, espagnol, allemand, italien, portugais, chinois et japonais. La langue de Windows est choisie automatiquement ; *Réglages › Langue* permet d'en changer, et le choix est retenu.
- Le menu de la zone de notification et le choix de dossier suivent la langue choisie ; l'installateur, ses raccourcis et l'inscription dans *Paramètres › Applications* sont dans la langue de Windows.
- Messages d'erreur du moteur audio et de l'installateur traduits, détail technique conservé.
- Seules les traductions de la langue choisie sont chargées.

## 0.2.0 — 2026-11-05 — Publication open source

- Code publié sous licence GNU GPL v3 ou ultérieure.
- **Installateur** : la version déjà installée est indiquée et remplacée proprement (fichiers de l'ancienne version retirés, inscription Windows réécrite) ; bibliothèque, réglages et connexion Apple Music conservés.
- Crédit « jeremdevgit » dans l'application, l'installateur, *Paramètres › Applications* et les propriétés du fichier.
- Réglages : nouvelle section **Mes autres logiciels**, avec le lien de téléchargement officiel de chacun.
- Versions construites par GitHub Actions, avec contrôle automatique du binaire.

## 0.1.0

- Première version : Apple Music et musique locale, paroles, égaliseur, mini-lecteur, statut Discord, installateur intégré.
