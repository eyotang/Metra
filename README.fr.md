# Metra

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · **Français** · [Deutsch](README.de.md) · [Русский](README.ru.md)

Metra suit l’utilisation de Cursor, Codex et Claude Code avec **Tauri 2, TypeScript et Rust**. Windows utilise WebView2, macOS WKWebView et Linux WebKitGTK. Les surfaces sont opaques, sans flou, avec des contours lisses.

La bulle flottante de 56 px peut être déplacée et se replier en capsule de 32 px au bord de l’écran. Le survol la déploie ; un clic ou Entrée ouvre les détails. Le mode barre d’état alterne les services visibles toutes les cinq secondes. Les détails présentent les comptes, limites, réinitialisations, tokens et coûts. Les paramètres conservent l’ordre, la visibilité, les libellés, les couleurs, la langue, l’intervalle et le lancement au démarrage. Sept langues sont disponibles : anglais, chinois simplifié, japonais, coréen, français, allemand et russe, avec détection automatique.

Les paramètres existants dans `dev.metra.desktop/settings.db` sont conservés. La lecture des identifiants Cursor nécessite un consentement explicite. En cas d’échec d’actualisation, les dernières données restent affichées avec un indicateur d’obsolescence.

## Développement

Installez Rust stable, Node.js 24+ et les prérequis Tauri de votre système : outils en ligne de commande Xcode sur macOS ; outils C++ Visual Studio, Windows SDK et WebView2 sur Windows ; WebKitGTK 4.1, GTK 3 et AppIndicator sur Linux (X11/XWayland pour les fenêtres positionnées).

```sh
npm ci
npm run dev
npm run dev -- -- --settings
npm run frontend:build
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
npm run build -- --no-bundle -- --locked
```

`METRA_CONFIG_DIR` permet d’isoler les paramètres de test. Fermer un panneau laisse Metra actif ; utilisez le menu pour quitter. Consultez la [configuration Linux](docs/linux.md).

Le collecteur Omarchy fonctionne sans Node ni dépendances de bureau :

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
target/collector/release/metra --watch-json
```

Le mode de surveillance accepte `refresh` sur l’entrée standard et s’arrête lorsqu’elle se ferme. Voir le [guide Omarchy](integrations/omarchy/README.md).

## Distribution et mises à jour

```sh
node scripts/package-native.mjs
# DMG universel macOS (Apple Silicon et Intel) :
rustup target add aarch64-apple-darwin x86_64-apple-darwin
node scripts/package-native.mjs --universal
```

Les fichiers sont placés dans `release-assets/` : DMG macOS, installateur NSIS par utilisateur et EXE portable Windows, paquet Debian Linux. La version portable Windows nécessite WebView2. `CARGO_TARGET_DIR` est pris en charge.

Pour une installation locale macOS, construisez l’application puis exécutez `npm run install:local` : sauvegarde de l’ancienne application, conservation des paramètres, suppression des attributs étendus et vérification de signature. Pour une application téléchargée, déplacez-la dans Applications puis exécutez avant le premier lancement :

```sh
xattr -cr /Applications/Metra.app
```

Les mises à jour utilisent `latest.json` et la clé publique Minisign existante. Sur Windows installé, une action explicite vérifie la signature et lance l’installateur. Les versions portables, macOS et Linux ouvrent la page de téléchargement pour une installation manuelle.

Voir le [README anglais](README.md) pour l’architecture et les vérifications CI. [Licence MIT](LICENSE) ; Tauri est sous MIT/Apache-2.0 et Inter conserve sa licence OFL dans `assets/fonts/`.
