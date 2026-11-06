# Instructions pour les agents — MPTE

Ces consignes s’appliquent à l’ensemble du dépôt. Lire aussi le [README](README.md)
avant de modifier le comportement de l’application. Les demandes explicites de
l’utilisateur priment sur les conventions de ce fichier.

## Objectif du produit

MPTE est un éditeur de texte pour macOS, rapide et léger, inspiré de Notepad++
avec moins de réglages. Il utilise Rust et egui/eframe avec OpenGL. Conserver cette
architecture ; toute nouvelle dépendance doit répondre à un besoin concret.

- Écrire les libellés, messages et explications à l’utilisateur en français.
- Garder l’édition au centre de l’interface, avec tout l’espace disponible.
- Préserver les onglets, leur détachement dans des fenêtres et les raccourcis Mac.
  Sous Linux, Ctrl remplace Cmd ; Alt/Option sert à la sélection en colonne.
- Conserver le thème clair, les champs bordés et les boutons cohérents. Utiliser
  les composants existants et vérifier les fenêtres étroites.
- La recherche avancée reste un dialogue séparé de l’éditeur, à trois onglets.
  Garder les options et les modes affichés sans section de réglages repliée.
  La barre de recherche rapide conserve son rôle distinct.
- Pour une retouche visuelle, réutiliser les commandes et le moteur existants.
  Limiter les changements au périmètre demandé.

## Carte du code

| Fichier | Responsabilité |
| --- | --- |
| `src/main.rs` | État partagé, documents, fenêtres, commandes et raccourcis |
| `src/document.rs` | Chargement, état modifié, encodage et sauvegarde |
| `src/editor.rs` | Rendu du texte, curseur, sélections et annulation |
| `src/column.rs` | Sélection rectangulaire et édition en colonnes |
| `src/outline.rs` | Délimiteurs, niveaux et repliement des blocs |
| `src/theme.rs`, `src/chrome.rs` | Thème, icônes, barre d’outils, onglets et état |
| `src/search.rs` | Motifs, résultats, recherche dans les fichiers et remplacements |
| `src/search_ui.rs` | État et commandes de recherche, barre rapide |
| `src/search_ui/compact.rs` | Présentation du dialogue de recherche avancée |
| `src/formatter.rs` | Formatage JSON et appel des formateurs externes |
| `tools/format.mjs` | Prettier et extensions HTML, XML et Java |
| `scripts/package-macos.sh` | Construction du bundle macOS avec Node et les formateurs |

## Préparer et lancer

Utiliser la version Rust de `rust-toolchain.toml` et la version Node de `.nvmrc`
(actuellement Rust 1.88.0 et Node 24). Garder les fichiers de verrouillage à jour
avec leurs manifestes. Ne pas mettre npm à jour globalement pour installer le projet.

Depuis la racine du dépôt :

```sh
npm ci --prefix tools
cargo run --locked
# Avec un fichier de test :
cargo run --locked -- /chemin/vers/exemple.json
```

Si Rust n’est pas sur le PATH dans le cloud et que les installations existent dans
`/workspace/.cargo` et `/workspace/.rustup`, activer cet environnement :

```sh
export CARGO_HOME=/workspace/.cargo RUSTUP_HOME=/workspace/.rustup
export PATH="$CARGO_HOME/bin:$PATH"
```

Pour évaluer le démarrage, utiliser le binaire compilé en release :

```sh
cargo build --release --locked
./target/release/mpte
```

## Comportements à préserver

- L’onglet actif « Sans titre » vide et non enregistré est supprimé lors de
  l’ouverture réussie d’un fichier ; préserver les autres documents et ne rien
  supprimer si l’ouverture échoue.
- Les documents sont en UTF-8. Conserver le BOM, les fins de ligne et les protections
  existantes contre l’écrasement de modifications externes. La sauvegarde est atomique.
- Les positions de recherche sont des plages d’octets UTF-8. Convertir explicitement
  entre octets, caractères et colonnes ; préserver les caractères composés et les tabulations.
- Le repliement ne supprime aucun texte. Sauvegarde et recherche utilisent le contenu
  complet ; atteindre un résultat masqué doit déplier son bloc.
- Toute édition ou remplacement dans un document ouvert doit rester annulable.
  Les remplacements dans les documents ouverts ne les enregistrent pas sur disque.
- Le remplacement dans un dossier garde son aperçu et sa confirmation, ignore les
  fichiers ouverts et vérifie les modifications externes avant l’écriture.
- Les tâches longues gardent leur traitement en arrière-plan. Un résultat de
  formatage devenu obsolète ne doit jamais écraser une saisie plus récente.
- Le formatage à l’enregistrement est désactivé initialement et propre au document.
  JSON conserve l’ordre des clés et la précision des nombres, et refuse les clés dupliquées.

## Vérifier les changements

Pour une modification Rust, lancer :

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
```

Après modification des formateurs ou de leurs dépendances, lancer aussi :

```sh
npm test --prefix tools
```

Ajouter un test de régression pertinent pour une correction de logique. Pour une
retouche d’interface, vérifier visuellement les états concernés, le focus, les
raccourcis et une largeur réduite. Utiliser des fichiers temporaires pour les essais
de remplacement ou de sauvegarde. Une modification documentaire seule demande une
relecture et `git diff --check`, sans recompilation systématique.

Le cloud Linux permet de vérifier le cœur et le rendu egui. Il ne valide pas le
comportement macOS, ses dialogues natifs ni son bundle. Signaler clairement ce qui
a été exécuté et ce qui reste à tester sur Mac. Le packaging s’exécute sur macOS
avec `bash scripts/package-macos.sh` ; le README détaille ses prérequis et limites.

## Livraison

- Préserver les modifications déjà présentes et éviter les refontes sans rapport.
- Mettre à jour le README si l’utilisation, les prérequis ou les limites changent.
- Ne pas versionner `target/`, `tools/node_modules/`, `dist/`, les captures de test
  ou des chemins et données personnels.
- Faire des commits ciblés et pousser lorsque la demande ou les instructions de
  la session l’autorisent. Ne pas forcer le push ni réécrire l’historique partagé.
- Distinguer l’envoi du code sur GitHub, la publication de l’environnement cloud
  et la distribution d’une application macOS : ce sont des opérations distinctes.
- Dans le compte rendu, indiquer le résultat, les vérifications et, après un push,
  le commit ainsi que les commandes utiles pour tester sur Mac.
