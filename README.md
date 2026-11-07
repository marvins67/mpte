# MPTE

Un éditeur de texte pour macOS, rapide et léger, inspiré de la simplicité de Notepad++.
Rust + egui/eframe, rendu OpenGL, sans navigateur embarqué. Interface en français.

## Fonctionnalités

- Plusieurs fichiers dans des onglets. Un onglet peut être détaché dans une fenêtre indépendante.
- Ouverture multiple, glisser-déposer, fichiers passés en arguments, nouveau document, enregistrer et enregistrer sous.
- Numéros de ligne et coloration syntaxique selon l’extension, notamment JSON, XML, HTML et Java.
- Ligne du curseur surlignée, parenthèses/crochets/accolades associés en rouge.
- Repliement des blocs avec boutons +/− dans la marge et guides de niveaux.
- Sélection rectangulaire avec Alt/Option + glisser, copie et édition en colonnes.
- Zone d’édition étendue à toute la largeur et toute la hauteur disponibles.
- Interface claire avec une barre d’outils compacte à boutons et des onglets soulignés en bleu.
- Recherche rapide intégrée et boîte de recherche avancée à trois onglets, accessible par Cmd+F.
- Taille de police de l’éditeur réglable au clavier et mémorisée entre les lancements.
- Formatage à la demande ou à l’enregistrement (case propre à chaque document, désactivée initialement).
- Confirmation avant fermeture des documents modifiés, annuler/rétablir via le composant de texte.
- Fichiers UTF-8 uniquement, conservation du BOM et des fins de ligne CRLF, sauvegarde atomique, détection des modifications externes.

JSON est formaté en Rust, avec ordre des clés et précision numérique conservés.
HTML, XML et Java utilisent Prettier et ses extensions officielles/communautaires,
dans un processus Node séparé. Le formatage à la demande et lors d’un enregistrement habituel
s’exécute en arrière-plan. Les modifications faites entre-temps sont préservées.
Enregistrer sous et enregistrer lors de la fermeture attendent le formateur.
Une erreur de syntaxe est affichée et empêche la sauvegarde avec formatage.

## Développement

Le développement et les tests utilisent Rust **1.88.0**, fixé dans
`rust-toolchain.toml`, et Node.js **24**, indiqué dans `.nvmrc`, avec le npm
fourni par Node.js. Sur macOS, installer les outils de ligne de commande Xcode
avec `xcode-select --install`. Avec nvm déjà installé, `nvm install` puis
`nvm use` activent la version Node.js du projet.

Depuis la racine du dépôt, installer les formateurs et lancer l’éditeur.

```sh
npm ci --prefix tools
cargo run --locked
# Ouvrir des documents dès le lancement
cargo run --locked -- exemple.json Exemple.java
```

Les lancements avec `cargo run` utilisent Node.js et les formateurs installés
dans l’environnement de développement. La création de l’application autonome
est décrite dans la section [Application macOS](#application-macos).

## Raccourcis et préférences

Sous Linux, Ctrl remplace Cmd.

| Raccourci macOS | Action |
| --- | --- |
| Cmd+N | Nouveau document |
| Cmd+O | Ouvrir des fichiers |
| Cmd+S | Enregistrer |
| Cmd+Maj+S | Enregistrer sous |
| Cmd+W | Fermer l’onglet actif |
| Cmd+Maj+N | Nouvelle fenêtre |
| Cmd+Maj+F | Formater le document |
| Cmd+F | Ouvrir la recherche avancée |
| `Cmd` + `+` ou `Cmd` + `=` | Augmenter la police de l’éditeur d’un point |
| `Cmd` + `−` | Réduire la police de l’éditeur d’un point |
| Cmd+0 | Rétablir la police de l’éditeur à 14 points |
| Cmd+Q | Quitter MPTE |

Couper, copier, coller, sélectionner, annuler et rétablir utilisent les
raccourcis habituels du composant de texte.

La taille de police s’applique au texte de l’éditeur et aux numéros de ligne,
dans tous les documents et toutes les fenêtres. Elle varie de **8 à 72 points**.
Les boutons, les onglets et les dialogues conservent leur taille lorsque ces
raccourcis sont utilisés.

Cette taille est enregistrée automatiquement et à la fermeture de l’application,
puis restaurée au lancement suivant. Sur macOS, elle est stockée dans
`~/Library/Application Support/MPTE/app.ron`, sous la clé `editor_font_size`.
Une ancienne préférence `zoom_factor` est convertie en taille de police.

Seule la taille de police est mémorisée. Chaque lancement sans fichier passé en
argument commence par un document **Sans titre**. Les documents ouverts lors de
la fermeture ne sont pas rouverts automatiquement.

## Vérifications

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
npm test --prefix tools
```

Pour évaluer le démarrage, compiler et lancer la version optimisée.

```sh
cargo build --release --locked
./target/release/mpte
```

## Interface et documents

La barre d’outils donne aussi accès à Enregistrer sous, Nouvelle fenêtre, Détacher
l’onglet, Fermer l’onglet, Recherche avancée et Quitter. Si la fenêtre rétrécit,
les libellés disparaissent progressivement. Toutes les commandes restent accessibles
par leur icône, avec une info-bulle au survol. Un clic droit sur un onglet permet
également de le détacher. Le bouton de fermeture et le clic du milieu ferment l’onglet.
L’ouverture réussie d’un fichier remplace l’onglet actif **Sans titre** lorsqu’il
est vide et non enregistré.
La barre d’état réunit ligne/colonne, nombre de lignes, formatage à l’enregistrement,
encodage et type de fichier. Les commandes de fenêtre restent celles du système
d’exploitation.

## Rechercher et remplacer

Le bouton **Rechercher** de la barre d’outils ouvre une barre intégrée sous les
onglets. La saisie surligne les occurrences dans le fichier actif et affiche un
compteur. Les flèches ou Entrée / Maj+Entrée passent à l’occurrence suivante ou
précédente. Échap ou la croix ferme la barre. Le bouton de réglages ouvre les options
avancées. La recherche rapide utilise du texte littéral, sans distinction de casse.

Cmd+F ouvre un dialogue séparé de l’éditeur, au format compact, qui réunit les
onglets **Rechercher**, **Remplacer** et **Dans les fichiers**. Les champs sont bordés, les
options et les trois modes restent visibles, et les actions partagent le même
style. Les deux groupes de réglages s’empilent lorsque la fenêtre est étroite.
Entrée conserve l’action suivante (ou lance la recherche dans les fichiers).
Échap ferme d’abord l’historique ouvert, puis le dialogue.
La recherche propose le respect de la casse, les mots entiers, le sens arrière,
le bouclage et un périmètre limité au texte sélectionné avant l’ouverture du dialogue.
Les résultats dans le document actif ou les documents ouverts sont cliquables,
y compris pour les documents non enregistrés et les fenêtres détachées.

Les trois modes disponibles sont normal (texte littéral), étendu (`\n`, `\r`, `\t`,
`\\`, `\xNN`, `\uNNNN`) et expression régulière. Les regex utilisent la syntaxe du
crate Rust `regex`. Le remplacement accepte les groupes `$1` et `${nom}`, sans assertions
avant/arrière ni références arrière dans le motif. `^` et `$` délimitent les lignes.
Une case permet au point de correspondre aussi aux fins de ligne. Les fins de ligne
des documents sont normalisées en `\n` pendant l’édition.

La recherche dans les fichiers accepte un dossier et des filtres séparés par `;`
(ex. `*.json;*.xml`). Elle s’exécute en arrière-plan, avec sous-dossiers et éléments
cachés en option. Les liens symboliques sont ignorés. Les fichiers binaires,
non UTF-8 ou dépassant 16 Mio sont ignorés. L’affichage est limité à 5 000 résultats.
Un clic ouvre le résultat et sélectionne l’occurrence. Si le contenu a changé,
il faut relancer la recherche.

Les remplacements dans les documents ouverts restent **non enregistrés**. Le
remplacement dans un dossier prépare d’abord un aperçu, puis exige une confirmation
avant d’écrire sur disque. Les fichiers ouverts dans MPTE sont ignorés par cette
opération. Pour les traiter, utiliser le remplacement dans tous les documents ouverts.
Les changements externes sont détectés avant chaque écriture. Le plan est limité
à 100 Mio de texte. Après une annulation ou une erreur, les fichiers déjà
enregistrés conservent les modifications effectuées. Le détail des erreurs est affiché dans le dialogue.

## Navigation dans les blocs

Les boutons − et + de la marge ferment et ouvrent les blocs multilignes. Les
numéros de ligne restent ceux du fichier original. Les délimiteurs `{}`, `[]`
et `()` sont associés en ignorant les chaînes et commentaires courants. Les
balises XML/HTML multilignes peuvent également être repliées.

Le repliement est propre à chaque document. L’enregistrement, la copie, la
recherche et le remplacement utilisent le texte complet. Une recherche ou un
déplacement du curseur dans une partie masquée la déplie automatiquement. Une modification du texte réinitialise les replis.

## Sélection en mode colonne

Maintenir **Alt** (touche **Option ⌥** sur Mac) pendant un glisser de souris dans
le texte sélectionne un rectangle. Relâcher la touche conserve cette sélection.
La barre d’état indique le nombre de lignes et de colonnes sélectionnées.

- Cmd+C et Cmd+X copient ou coupent uniquement les colonnes sélectionnées.
- Saisir du texte remplace le bloc sur chaque ligne. La saisie suivante continue
  aux mêmes colonnes sur toutes les lignes. Retour arrière et Suppr y agissent aussi.
- Coller dans un rectangle répartit un texte multiligne ligne par ligne. Une
  seule ligne collée se répète sur chaque ligne sélectionnée. Les lignes courtes
  sont complétées par des espaces lorsque l’insertion le nécessite.
- Cmd+Z annule les modifications. Échap, un clic normal ou une touche de navigation
  quitte le mode colonne. La sélection classique reste disponible sans Alt.

Les tabulations occupent quatre colonnes, comme dans l’éditeur. Une sélection
partielle d’une tabulation est copiée en espaces. Les caractères composés sont
préservés et les lignes masquées par le repliement sont exclues du rectangle.
La copie vers une autre application utilise du texte ordinaire, avec une ligne
par rangée du bloc. Le collage hors d’un rectangle reste un collage de texte normal.

## Application macOS

La création du bundle s’exécute sur un Mac avec Rust et les outils de ligne de
commande Xcode. Depuis la racine du dépôt, lancer les commandes suivantes.

```sh
bash scripts/package-macos.sh && open dist/MPTE.app
```

Le script télécharge le binaire officiel de Node.js pour l’architecture du Mac
et la version majeure indiquée dans `.nvmrc`. Il vérifie son empreinte SHA-256,
puis l’utilise pour installer les formateurs et l’intègre au bundle avec sa licence.
Les archives vérifiées sont conservées dans `target/node-macos/` pour les
constructions suivantes. Une connexion Internet est nécessaire à la construction.

L’application produite requiert macOS 13.5 ou ultérieur, comme Node.js 24.
Elle contient Node.js et les formateurs et peut fonctionner hors ligne.
Pour reconstruire une application existante après une mise à jour du code,
quitter MPTE et déplacer le précédent bundle avant de relancer le script.

```sh
git pull --ff-only origin main
if [ -d dist/MPTE.app ]; then
  mv dist/MPTE.app "dist/MPTE-ancien-$(date +%Y%m%d-%H%M%S).app"
fi
bash scripts/package-macos.sh && open dist/MPTE.app
```

L’application créée se trouve dans `dist/MPTE.app`. Elle peut ensuite être
déplacée dans le dossier **Applications**.

Le workflow GitHub Actions **Application macOS Intel** construit également un
bundle Intel sur macOS à chaque modification du code ou de l’empaquetage sur
`main`. Son artefact **MPTE-macOS-Intel** contient l’archive de `MPTE.app`.
Les formateurs embarqués y sont vérifiés avec le seul PATH système.

Le script embarque l’icône « La ligne active » pour le Finder et le Dock. Sa
source est `assets/MPTE.svg` et le bundle utilise `assets/MPTE.icns`.
Le bundle contient le binaire Rust, Node.js et les formateurs pour un lancement
depuis le Finder avec toutes ses dépendances applicatives. Une construction locale
cible l’architecture du Mac utilisé, Intel ou Apple Silicon. Le script applique
une signature ad hoc et la vérifie avec `codesign`. La signature Developer ID et
la notarisation Apple restent à ajouter pour la distribution.

## Limites de cette base

- Pas encore d’intégration aux menus natifs macOS ni d’association Finder « Ouvrir avec ».
- Pas encore de mesures de démarrage, mémoire ou performances sur de gros fichiers.
  La coloration est désactivée au-delà de 2 Mio. Le composant charge le document entier.
- Le bundle inclut Node.js et les formateurs, ce qui augmente sa taille.
- Le formateur JSON refuse les clés dupliquées pour préserver les données.
- La sauvegarde atomique préserve les permissions Unix mais pas encore les attributs étendus macOS.
- Enregistrer régulièrement, car un arrêt brutal peut entraîner la perte des modifications en cours.

Le workflow **Check** exécute le formatage, Clippy, les tests Rust et les tests des
formateurs sur Linux et macOS. Le workflow **Application macOS Intel** vérifie
également la construction du bundle, sa signature et les formateurs embarqués.
Les interactions de l’interface, les dialogues natifs et le lancement depuis le
Finder et le Dock font l’objet d’essais manuels sur Mac.
