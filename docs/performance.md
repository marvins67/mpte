# Performances : correctifs et vérification

Les changements du 15 novembre 2026 traitent la boucle de rafraîchissement, les
copies et données retenues après fermeture, la mise en page Partition, les parcours
Unicode, les recherches et les formateurs. Aucun changement de moteur graphique.

## Vérification du repos

Un même test egui sans affichage natif, exécuté sur le commit d’origine `c6a1f95`
puis sur le code corrigé, dessine 10 images de stabilisation et observe les 20
suivantes, sans focus ni entrée utilisateur :

| Code | Images demandant immédiatement un autre affichage |
| --- | --- |
| Avant correction | 20 / 20 |
| Après correction | 0 / 20 |

Ce résultat vérifie la suppression de la boucle logicielle. Il ne mesure ni les
images réellement présentées par macOS, ni les watts, ni les ventilateurs.

Les tests permanents couvrent le repos normal et Partition, les titres, les
fenêtres détachées, la libération du cache et de l’annulation à la fermeture, la
réutilisation de la pagination, les changements de typographie, les index UTF-8,
l’annulation vers un état enregistré, les recherches fréquentes et les erreurs de
limite mémoire ou de durée des formateurs. Les tests existants vérifient aussi la
sauvegarde atomique, les modifications externes et les résultats périmés.

## Comparaison sur Mac

Utiliser le même document et les mêmes fenêtres, dans une application Release.
Attendre une minute après ouverture ; observer deux minutes sans interaction.
Comparer les états suivants avec l’ancienne version :

- Fenêtre active, puis autre application au premier plan.
- MPTE masqué, minimisé, puis plusieurs fenêtres détachées.
- Texte normal, gros JSON/XML, puis Partition avec plusieurs pages.
- Ouverture et fermeture répétées de gros documents, recherche fréquente et annulation.
- Écran éteint et vraie veille système, puis le même essai avec MPTE quitté.

Relever CPU, mémoire, impact énergétique, App Nap et prévention de veille dans
Moniteur d’activité. Utiliser « Échantillonner le processus » pendant une anomalie.
La mémoire résidente peut rester élevée après libération à cause de l’allocateur ;
Instruments Allocations permet de distinguer allocations vivantes et mémoire réservée.

Ces commandes de lecture permettent de comparer les assertions et événements :

```sh
pmset -g assertions
pmset -g log > ~/Desktop/MPTE-veille.txt
```

Noter les heures exactes et distinguer écran éteint, vraie veille et réveil de
maintenance. Aucune attribution définitive des ventilateurs à MPTE n’est faite
sans ces relevés.

## Limites et pistes qui demandent un travail supplémentaire

- Le composant TextEdit traite toujours le texte entier ; la coloration et l’analyse
  des blocs sont recalculées après édition. Les fichiers très volumineux nécessitent
  encore des mesures avant une analyse incrémentale ou une virtualisation complète.
- L’édition en colonne reconstruit encore le texte. Les copies nécessaires à
  l’annulation native egui et à la protection des sauvegardes restent présentes.
- Certaines ouvertures et sauvegardes, Enregistrer sous et la sauvegarde à la fermeture
  restent synchrones. Leur déplacement en arrière-plan nécessite de revoir le flux
  de fermeture et ses confirmations pour préserver les protections actuelles.
- OpenGL et la propagation des événements d’occultation/veille par les dépendances
  doivent être vérifiés sur macOS. Aucun branchement natif non validé ni changement
  de backend n’a été ajouté.
- La fermeture d’un dialogue de recherche ne signifie pas annulation d’une écriture
  explicitement confirmée. Le bouton Annuler conserve son rôle ; les contrôles
  d’annulation sont maintenant effectués entre les occurrences et les remplacements.
- Le délai des formateurs borne leur durée ; il n’ajoute pas un bouton d’annulation
  du formatage et ne constitue pas un ordonnanceur global des tâches.

La limite de 100 Mio d’un plan de remplacement concerne les données conservées,
pas le pic total du processus : il existe des allocations transitoires et les
compagnons Partition d’un document remplaçable sont toujours chargés pour préserver
leur mise en page et les protections contre les modifications externes.
