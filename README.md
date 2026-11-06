# MPTE

Un premier éditeur de texte pour macOS inspiré de la simplicité de Notepad++.
Rust + egui/eframe, rendu OpenGL, sans navigateur embarqué. Interface en français.

## Première version

- Plusieurs fichiers dans des onglets ; un onglet peut être détaché dans une fenêtre indépendante.
- Ouverture multiple, glisser-déposer, fichiers passés en arguments, nouveau document, enregistrer et enregistrer sous.
- Numéros de ligne, coloration JSON, XML, HTML et Java suivant l’extension.
- Ligne du curseur surlignée, parenthèses/crochets/accolades associés en rouge.
- Repliement des blocs avec boutons +/− dans la marge et guides de niveaux.
- Sélection rectangulaire avec Alt/Option + glisser, copie et édition en colonnes.
- Zone d’édition étendue à toute la largeur et toute la hauteur disponibles.
- Interface claire avec icônes, onglets soulignés en bleu et menu « … » pour les actions secondaires.
- Recherche rapide intégrée et boîte de recherche avancée à trois onglets, accessible par Cmd+F.
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
(24 LTS recommandé, version indiquée dans `.nvmrc`) et npm fourni avec Node.
Sur macOS : outils de ligne de commande Xcode (`xcode-select --install`).
Il n’est pas nécessaire de mettre npm à jour séparément. Si npm refuse une mise à
jour avec `EBADENGINE`, conserver le npm fourni avec Node ou passer à Node 24 LTS.
Avec nvm déjà installé : `nvm install` puis `nvm use` depuis la racine du dépôt.

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
Cmd+F ouvre la recherche (Ctrl+F sous Linux).
Couper/copier/coller, sélection et annulation utilisent les raccourcis habituels du composant.

```sh
npm test --prefix tools
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

Pour tester la vitesse de lancement sans passer par la compilation de développement :

```sh
cargo build --release --locked
./target/release/mpte
```

## Rechercher et remplacer

Le bouton **Rechercher** de la barre d’outils ouvre une barre intégrée sous les
onglets. La saisie surligne les occurrences dans le fichier actif et affiche un
compteur. Les flèches ou Entrée / Maj+Entrée passent à l’occurrence suivante ou
précédente. Échap ou la croix ferme la barre. L’icône de réglages ouvre les options
avancées ; la recherche rapide utilise du texte littéral, sans distinction de casse.

Cmd+F ouvre un dialogue séparé de l’éditeur, au format compact, avec trois onglets :
**Rechercher**, **Remplacer**, **Dans les fichiers**. Les champs sont bordés, les
options et les trois modes restent visibles, et les actions partagent le même
style. Les deux groupes de réglages s’empilent lorsque la fenêtre est étroite.
Entrée conserve l’action suivante (ou lance la recherche dans les fichiers) ;
Échap ferme d’abord l’historique ouvert, puis le dialogue.
La recherche propose le respect de la casse, les mots entiers, le sens arrière,
le bouclage et un périmètre limité au texte sélectionné avant l’ouverture du dialogue.
Les résultats dans le document actif ou les documents ouverts sont cliquables,
y compris pour les documents non enregistrés et les fenêtres détachées.

Trois modes sont disponibles : normal (texte littéral), étendu (`\n`, `\r`, `\t`,
`\\`, `\xNN`, `\uNNNN`) et expression régulière. Les regex utilisent la syntaxe du
crate Rust `regex` : groupes `$1` et `${nom}` pour le remplacement, sans assertions
avant/arrière ni références arrière dans le motif. `^` et `$` délimitent les lignes ;
une case permet au point de correspondre aussi aux fins de ligne. Les fins de ligne
des documents sont normalisées en `\n` pendant l’édition.

La recherche dans les fichiers accepte un dossier et des filtres séparés par `;`
(ex. `*.json;*.xml`). Elle s’exécute en arrière-plan, avec sous-dossiers et éléments
cachés en option. Les liens symboliques ne sont pas suivis ; les fichiers binaires,
non UTF-8 ou dépassant 16 Mio sont ignorés. L’affichage est limité à 5 000 résultats.
Un clic ouvre le résultat et sélectionne l’occurrence ; un contenu devenu différent
demande de relancer la recherche.

Les remplacements dans les documents ouverts restent **non enregistrés**. Le
remplacement dans un dossier prépare d’abord un aperçu, puis exige une confirmation
avant d’écrire sur disque. Les fichiers ouverts dans MPTE sont ignorés par cette
opération : utiliser le remplacement dans tous les documents ouverts pour ceux-ci.
Les changements externes sont détectés avant chaque écriture. Le plan est limité
à 100 Mio de texte ; une annulation ou une erreur ne revient pas sur les fichiers
déjà enregistrés. Le détail des erreurs est affiché dans le dialogue.

Le menu **…** regroupe Enregistrer sous, Nouvelle fenêtre, Détacher l’onglet,
Recherche avancée, Fermer l’onglet et Quitter. Un clic droit sur un onglet permet
aussi de le détacher ; le bouton de fermeture et le clic du milieu ferment l’onglet.
La barre d’état réunit ligne/colonne, nombre de lignes, formatage à l’enregistrement,
encodage et type de fichier. La barre d’outils passe aux icônes seules si la fenêtre
est étroite. Les commandes de fenêtre restent celles du système d’exploitation.

## Navigation dans les blocs

Les boutons − et + de la marge ferment et ouvrent les blocs multilignes. Les
numéros de ligne restent ceux du fichier original. Les délimiteurs `{}`, `[]`
et `()` sont associés en ignorant les chaînes et commentaires courants ; les
balises XML/HTML multilignes peuvent également être repliées.

Le repliement est propre à chaque document et conserve son texte complet :
enregistrement, copie, recherche et remplacement portent toujours sur le contenu
intégral. Une recherche ou un déplacement du curseur dans une partie masquée
la déplie automatiquement. Une modification du texte réinitialise les replis.

## Sélection en mode colonne

Maintenir **Alt** (touche **Option ⌥** sur Mac) pendant un glisser de souris dans
le texte sélectionne un rectangle. Relâcher la touche conserve cette sélection.
La barre d’état indique le nombre de lignes et de colonnes sélectionnées.

- Cmd+C et Cmd+X copient ou coupent uniquement les colonnes sélectionnées.
- Saisir du texte remplace le bloc sur chaque ligne. La saisie suivante continue
  aux mêmes colonnes sur toutes les lignes ; Retour arrière et Suppr y agissent aussi.
- Coller dans un rectangle répartit un texte multiligne ligne par ligne ; une
  seule ligne collée se répète sur chaque ligne sélectionnée. Les lignes courtes
  sont complétées par des espaces lorsque l’insertion le nécessite.
- Cmd+Z annule les modifications. Échap, un clic normal ou une touche de navigation
  quitte le mode colonne ; la sélection classique reste disponible sans Alt.

Les tabulations occupent quatre colonnes, comme dans l’éditeur. Une sélection
partielle d’une tabulation est copiée en espaces. Les caractères composés sont
préservés et les lignes masquées par le repliement ne font pas partie du rectangle.
La copie vers une autre application utilise du texte ordinaire, avec une ligne
par rangée du bloc. Le collage hors d’un rectangle reste un collage de texte normal.

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
- Pas de restauration de session à ce stade.
- Pas encore de mesures de démarrage, mémoire ou performances sur de gros fichiers.
  La coloration est désactivée au-delà de 2 Mio ; le composant charge le document entier.
- Le runtime Node embarqué augmente la taille du bundle. Pour une distribution plus légère,
  on pourra proposer le formatage Java/HTML/XML en module optionnel.
- Le formateur JSON refuse les clés dupliquées pour préserver les données.
- La sauvegarde atomique préserve les permissions Unix mais pas encore les attributs étendus macOS.
- Pas de récupération automatique après un arrêt brutal ; enregistrer régulièrement.

Les tests Linux vérifient le cœur et le rendu egui. Le comportement macOS, les
dialogues natifs et le bundle doivent encore être essayés sur un Mac.
