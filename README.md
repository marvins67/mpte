# MPTE

Un premier éditeur de texte pour macOS inspiré de la simplicité de Notepad++.
Rust + egui/eframe, rendu OpenGL, sans navigateur embarqué. Interface en français.

## Première version

- Plusieurs fichiers dans des onglets ; un onglet peut être détaché dans une fenêtre indépendante.
- Ouverture multiple, glisser-déposer, fichiers passés en arguments, nouveau document, enregistrer et enregistrer sous.
- Numéros de ligne, coloration JSON, XML, HTML et Java suivant l’extension.
- Formatage à la demande ou à l’enregistrement (case propre à chaque document, désactivée initialement).
- Confirmation avant fermeture des documents modifiés, annuler/rétablir via le composant de texte.
- Fichiers UTF-8 uniquement, conservation du BOM et des fins de ligne CRLF ; sauvegarde atomique, détection des modifications externes.

JSON est formaté en Rust, avec ordre des clés et précision numérique conservés.
HTML, XML et Java utilisent Prettier et ses extensions officielles/communautaires,
dans un processus Node séparé. Le formatage à la demande et à la sauvegarde habituelle
s’exécute en arrière-plan ; les modifications faites entre-temps ne sont jamais écrasées.
Enregistrer sous et enregistrer lors de la fermeture attendent le formateur.
Une erreur de syntaxe est affichée et empêche la sauvegarde avec formatage.

## Développement

Prérequis : Rust via rustup (version fixée dans `rust-toolchain.toml`), Node.js 22+
et npm. Sur macOS : outils de ligne de commande Xcode (`xcode-select --install`).

```sh
cd /chemin/vers/mpte
npm ci --prefix tools
cargo run --locked
# Ou ouvrir des documents dès le lancement :
cargo run --locked -- exemple.json Exemple.java
```

Dans le cloud actuel, activer Rust avant ces commandes :

```sh
export CARGO_HOME=/workspace/.cargo RUSTUP_HOME=/workspace/.rustup
export PATH="$CARGO_HOME/bin:$PATH"
cd /workspace/mpte
```

Raccourcis (Ctrl remplace Cmd sous Linux) : Cmd+N nouveau, Cmd+O ouvrir,
Cmd+S enregistrer, Cmd+Maj+S enregistrer sous, Cmd+W fermer l’onglet,
Cmd+Maj+N nouvelle fenêtre, Cmd+Maj+F formater, Cmd+Q quitter.
Couper/copier/coller, sélection et annulation utilisent les raccourcis habituels du composant.

```sh
npm test --prefix tools
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

## Application macOS

Sur un Mac, avec la distribution autonome officielle de Node.js sur le PATH :

```sh
bash scripts/package-macos.sh
open dist/MPTE.app
```

Le script embarque le binaire Rust, Node et les formateurs pour que le lancement
depuis le Finder ne dépende pas du PATH du terminal. Il construit pour l’architecture
du Mac actuel (Apple Silicon ou Intel), sans notarisation ; la signature ad hoc
convient aux essais locaux. Pour distribuer l’application, une signature Developer ID
et la notarisation Apple restent à ajouter. La fabrication du bundle doit être
validée sur macOS : ce cloud est Linux.

## Limites de cette base

- Pas encore d’intégration aux menus natifs macOS ni d’association Finder « Ouvrir avec ».
- Pas de recherche/remplacement ni de restauration de session à ce stade.
- Pas encore de mesures de démarrage, mémoire ou performances sur de gros fichiers.
  La coloration est désactivée au-delà de 2 Mio ; le composant charge le document entier.
- Le runtime Node embarqué augmente la taille du bundle. Pour une distribution plus légère,
  on pourra proposer le formatage Java/HTML/XML en module optionnel.
- Le formateur JSON refuse les clés dupliquées pour préserver les données.
- La sauvegarde atomique préserve les permissions Unix mais pas encore les attributs étendus macOS.
- Pas de récupération automatique après un arrêt brutal ; enregistrer régulièrement.

Les tests Linux vérifient le cœur et le rendu egui. Le comportement macOS, les
dialogues natifs et le bundle doivent encore être essayés sur un Mac.
