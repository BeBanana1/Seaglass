# Journal des versions

Une entrée par version de `VERSION`. Une version **publiée** porte un tag `v<version>` :
c'est une release (installeur et APK en ligne), qui contient aussi les versions
intermédiaires. Les versions d'avant le 2026-11-10 viennent de l'ancienne ligne « Statut »
de `CLAUDE.md` ; celles qui n'y figuraient pas reprennent le sujet des commits qui ont posé leur numéro
(en anglais au début du projet).

## 0.4.119

- Android : Retour ferme le mode conduite ouvert par le raccourci ou le lien au lieu de quitter l'app ; Retour est désormais demandé directement à l'interface (#193).

## 0.4.118

- Android : mode conduite plein écran (gros boutons de lecture, 1 à 8 tuiles qui lancent une playlist, un mix, les titres likés ou « Plus tard » en un appui), ouvert par l'icône voiture, le raccourci de l'app ou le lien `seaglass://drive` ; les mixes s'épinglent aussi dans la barre latérale du bureau (#193).

## 0.4.117

- Android : répétition et aléatoire demandés par un contrôleur (autoradio, montre) repartent vers Rust, et la session média ne propose plus de changer la liste de lecture, la vitesse ni le volume d'ExoPlayer (#254).

## 0.4.116

- Jam : limites comptées par invité (plafond 5 fois plus large par adresse), essais de code ratés oubliés au bout d'un quart d'heure, 100 titres de la Jam en attente au plus, code retenu par Jam dans la page ; relais à redéployer (#256).

## 0.4.115

- Tests du frontend avec `vitest` (tri et filtre des listes, formats, chaîne audio, liens, réorganisation, glissement), lancés par la CI ; rien ne change dans l'app (#264).

## 0.4.114

- Android : le titre en cours est mis en évidence dès son lancement, plus seulement après une pause (#291).

## 0.4.113

- Accueil personnalisé : le glisser-déposer des sections remarche sous Windows (la file et les playlists aussi), et sur Android une section se déplace par appui long puis en glissant le doigt (#290).

## 0.4.112

- Jam : contrôles de l'hôte (repliés) : limite de titres en attente par invité, albums permis ou non, ajouts suspendus, invités exclus ou réadmis ; un titre déjà dans la file est refusé (#288).

## 0.4.111

- mpv figé : arrêté après 5 s sans réponse au lieu de bloquer le lecteur ; mpv arrêté ou planté : relancé une fois et le titre reprend (#255).

## 0.4.110

- « En lecture » montre l'édition hi-res réellement lue, avec son format, et ouvre son album (#111).

## 0.4.109

- Session : une panne passagère de TIDAL n'efface plus le refresh token, une session non reprise au lancement est réessayée seule, et une session refusée en cours de route ramène à l'écran de connexion (#252, #259).

## 0.4.108

- Android : Retour ne quitte l'app que depuis l'accueil, et « En lecture » ouvert par la notification revient à la page d'où joue le titre (#240).

## 0.4.107

- Titre en cours plus visible dans les listes : fond teinté, barre d'accent et égaliseur (#241).

## 0.4.106

- Sections de l'accueil réordonnables par glisser-déposer dans « Personnaliser » (#246).

## 0.4.105

- Recherche lancée pendant la frappe, l'historique ne garde que la requête retenue (#248).

## 0.4.104

- Couleur d'accent gardée pour chaque thème (#250).

## 0.4.103

- « Plus tard » épinglable dans la barre latérale (#244).

## 0.4.102

- « Plus tard » : signet à côté du cœur pour retirer un titre en un clic (#249).

## 0.4.101

- Connexion en navigation privée, données de navigation effacées à la déconnexion (#258).

## 0.4.100

- Android : toasts au-dessus du clavier ouvert (#251).

## 0.4.99

- Android : tuile des paroles (et de la file) centrée en plein écran (#242).

## 0.4.98

- Android : affichage bloqué en portrait (#243).

## 0.4.97

- Petits défauts de l'audit : fragment de la page des invités, clients HTTP en HTTPS seulement, coupe de « feat. », chemins du `FileProvider` (#262).

## 0.4.96

- Dernière sauvegarde de la file, de la position et du volume à la fermeture et avant une mise à jour (#260).

## 0.4.95

- mpv du `PATH` utilisé seulement en debug ; bandeau « mpv introuvable » si le mpv embarqué manque (#261).

## 0.4.94 (publiée le 2026-11-06)

- Boutons à logos des apps de musique sur la page des invités de la Jam (#225).

## 0.4.93 (publiée le 2026-11-06)

- Liens `https://tidal.com/…` touchés ouverts dans l'app sur Android, à activer dans Réglages (#203).

## 0.4.92 (publiée le 2026-11-06)

- Présence Discord, « Écoute Seaglass » (#215).

## 0.4.91

- Lien de Spotify, Deezer, Apple Music, YouTube Music ou TIDAL collé dans la recherche de la Jam (#229), liens vers ces apps sur la page des invités (#225).

## 0.4.90

- Titres d'une recherche dont le nom correspond à la requête avant ceux des artistes de ce nom, dans l'app et la Jam (#232).

## 0.4.89

- Clic sur l'icône de la zone de notification qui cache la fenêtre déjà au premier plan (#226), bouton « Plus tard » dans l'en-tête d'un album (#227).

## 0.4.88

- Albums dans la Jam, et retrait de ses propres ajouts par un invité (#223).

## 0.4.87

- Lecture / pause par les invités de la Jam, au choix de l'hôte (#222).

## 0.4.86

- Croix pour effacer la recherche de la Jam (#224).

## 0.4.85 (publiée le 2026-11-06)

- Jam en accès rapide depuis « En lecture », délai de reprise du relais porté à 30 min (#25).

## 0.4.84

- Jam depuis Android, en ligne seulement (#25, étape 4, en cours de vérification sur le S21).

## 0.4.83

- Jam en ligne par le relais, sans Wi-Fi commun (#25, étape 3b), réseau local en second choix.

## 0.4.82

- Relais de la Jam (#25, étape 3a, `relay/`, à déployer), page des invités en chemins relatifs.

## 0.4.81

- Jam locale, étape 2 (#25) : QR code, prénom de l'invité dans la file, état poussé en direct aux invités.

## 0.4.80

- « Plus tard » (Save for later de TIDAL), onglet de la bibliothèque et entrée des menus (#196).

## 0.4.79

- Jam locale, étape 1 (#25) : des invités sur le même réseau ajoutent des titres à la file depuis une page web.

## 0.4.78 (publiée le 2026-11-05)

- Gérer ses playlists, réordonner, renommer, supprimer (#201), historique d'écoute local et « Écoutés dans Seaglass » (#202), lien partagé vers l'app sur Android (#203, à vérifier sur le S21), mixes dans la bibliothèque (#204), page d'artiste complète (#205).

## 0.4.77

- Accueil complet, pages suivantes du flux (#207).

## 0.4.76

- Historique de recherche (#211).

## 0.4.75

- Lien TIDAL collé ouvert dans l'app, sur le bureau (#203), crédits d'un album entier (#206).

## 0.4.74

- En-tête des pages centré sur Android, Lecture / Aléatoire puis les autres boutons sur deux lignes, filtre pleine largeur et tri en un bouton (#182).

## 0.4.73

- Nom de l'app aligné sur les boutons de l'en-tête, centré avec la version dessous sur Android (#183).

## 0.4.72

- Playlists et onglets de la bibliothèque gardés d'un lancement à l'autre, longues listes affichées par morceaux (#181).

## 0.4.71

- Bandeau « À la une » tournant, masquable et passable (#185), lien vers le dépôt et les issues dans À propos (#188), vérification des mises à jour du bureau sans attendre le téléchargement (#187), images de l'installeur (#194).

## 0.4.70 (publiée le 2026-11-05)

- Bouton Radio du titre dans « En lecture » (#179), artistes non cliquables dans les lignes sur Android (#180), bandeau « À la une » sans « Ouvrir l'album » et plus bas sur Android (#184), langue du système lue dans `java.util.Locale` sur Android (#186), « Se déconnecter » sous À propos en `--danger`, plus dans l'en-tête du bureau (#189), « En lecture » compacte sur Android, sans étiquette (#195).

## 0.4.69 (publiée le 2026-11-04)

- Glisser un titre vers la droite l'ajoute à la file sur Android (#174).

## 0.4.68

- Paroles et file en plein écran dans « En lecture » sur Android (#173).

## 0.4.67

- Thèmes Alliage clair et sombre (#175).

## 0.4.66 (publiée le 2026-11-04)

- Titres de la file cliquables, lecture depuis la file (#167).

## 0.4.65

- Plus de plantage « no available activity » quand on pilote la lecture après avoir balayé l'app, commandes du lecteur en JNI direct (#169).

## 0.4.64

- « Précédent » ne remplit plus la file manuelle de titres que l'utilisateur n'y a jamais mis (#166).

## 0.4.63

- Notification d'Android cliquable (ouvre « En lecture ») avec boutons aléatoire et répétition (#163).

## 0.4.62

- Lecture / Aléatoire dans les titres likés (#161), plus de numéros dans les playlists et mixes sur Android (#162).

## 0.4.61

- Aucun fond ni décor animé sur Android (#158).

## 0.4.60

- L'APK porte la version de `VERSION` au lieu de 1.0 (#157).

## 0.4.59

- Playlists rouvertes sans « Chargement… », revalidées par ETag (#152).

## 0.4.58 (publiée le 2026-11-04)

- Mode recherche dans une playlist et les titres likés sur Android (#151).

## 0.4.57

- Audit de sécurité et de performances, surtout Android (#144 à #148, voir « Audit (0.4.57) »).

## 0.4.56

- Messages éphémères centrés au-dessus de la lecture, sur une surface pleine, visibles dans « En lecture » (#150).

## 0.4.55

- Onglet « Titres likés » dans la navigation d'Android, réglages en haut à droite (#143).

## 0.4.54 (publiée le 2026-11-03)

- L'installeur fait relire l'icône à Windows (#139).

## 0.4.53

- Builds de release plus rapides (ThinLTO, plus de `staticlib`), avertissements du build Android traités.

## 0.4.52 (publiée le 2026-11-03)

- Nouveau logo, vinyle soleil et mer, aussi sur Android (#126).

## 0.4.51

- Vrai plein écran (F11) (#131).

## 0.4.49

- « Joué depuis » dans « En lecture » (#124).

## 0.4.48

- Paroles et file côte à côte dans « En lecture » (#119).

## 0.4.47 (publiée le 2026-11-03)

- APK release signé et mises à jour sur Android (#24).

## 0.4.46

- Mise en page tactile sur Android (#24).

## 0.4.45

- Refresh token dans l'Android Keystore (#24).

## 0.4.44

- Lecture Media3 et session média sur Android (#24).

## 0.4.43 (publiée le 2026-11-03)

- Squelette Android, sans lecture, avec connexion (#24).

## 0.4.42

- Moteur de lecture abstrait, première étape de l'app Android (#24).

## 0.4.41 (publiée le 2026-11-03)

- (pas de note dans l'ancien statut)

## 0.4.40

- feat(integrations): icône de la zone de notification et fermeture réduite (#83)

## 0.4.37

- feat(library): tri mémorisé par playlist et pour les titres likés (#93)

## 0.4.36

- perf(ui): vignettes des listes en 160 px (#89)

## 0.4.35

- Thème Aéro (#101).

## 0.4.30

- chore: version 0.4.30 (#91)

## 0.4.29

- chore: version 0.4.29 (#90)

## 0.4.28

- chore: version 0.4.28 (#78)

## 0.4.27

- chore: version 0.4.27 (#80)

## 0.4.26

- chore: version 0.4.26 (#92)

## 0.4.25

- feat(player): message clair quand le processeur n'a pas l'AVX2 exigé par mpv (#92)
- feat(player): choix du périphérique de sortie audio (#79)
- feat(library): favoris des albums, artistes et playlists (#81)
- feat(player): minuteur de mise en veille (#85)
- perf(ui): longues listes sans recalcul à chaque position (#88)
- feat(info): crédits du titre dans Infos du titre (#84)
- feat(player): normalisation du volume par ReplayGain (#82)

## 0.4.24 (publiée le 2026-10-30)

- Thème Bauhaus (#70).

## 0.4.23

- Thème Risographie (#69).

## 0.4.22

- Thème Terminal (#68).

## 0.4.21

- Thèmes Neumorphism clair et sombre (#67).

## 0.4.20

- Thème Cyberpunk (#66).

## 0.4.19

- Thème Solarpunk (#65).

## 0.4.18

- Jetons de thème, prérequis du lot de thèmes #62 (#64).

## 0.4.17

- Interface en anglais (#7).

## 0.4.16

- Images des mixes et radios (#56).

## 0.4.15 (publiée le 2026-10-30)

- Édition hi-res en mode « Demander » et lien vers l'album hi-res (#59).

## 0.4.14 (publiée le 2026-10-30)

- Infos du titre et débit dans la chaîne audio (#3).

## 0.4.13

- Édition hi-res trouvée aussi sur un album jumeau aux ISRC différentes (#55).

## 0.4.12 (publiée le 2026-10-30)

- Titres likés et ajout aux playlists (#4).

## 0.4.11

- Menu contextuel des titres (#52).

## 0.4.10 (publiée le 2026-10-30)

- Édition hi-res affichée en clair dans la chaîne audio (#18).

## 0.4.9

- Mixes mensuels dans le fil d'actualité (#14).

## 0.4.8

- Mixes et radios (#11), fil d'actualité (#14), paroles synchronisées (#5), édition hi-res d'un titre qui n'en a pas (#18).

## 0.4.7 (publiée le 2026-10-30)

- Installeur en français, page de téléchargement à `https://updates.bebanana.ch/seaglass/`.

## 0.4.6 (publiée le 2026-10-30)

- Playlists épinglées (#10), couleur d'accent (#23), vider la file (#41), bouton « Vérifier les mises à jour » (#43).

## 0.4.5 (publiée le 2026-10-30)

- Page Réglages (#9), volume (#29), cache mpv, lancement au démarrage (#13), accueil personnalisable (#21, #22), filtre et tri des listes (#1, #2), ajout d'albums à la file (#15), file dans « En lecture » (#20).

## 0.4.4 (publiée le 2026-10-29)

- Première version publiée : installeur NSIS signé, mpv embarqué, mises à jour automatiques.

## 0.4.3

- chore: release v0.4.3 (fixes #6, #8, #12, #27)

## 0.4.2

- chore: Gitea issues access for Claude sessions via gitea-mcp (v0.4.2)

## 0.4.1

- feat(ui): Acidwave and Surreal themes (v0.4.1)

## 0.4.0

- feat(ui): three themes, now-playing view and signal path (v0.4.0)

## 0.3.1

- feat(integrations): Windows media controls via SMTC (v0.3.1)

## 0.3.0

- feat(views): home, search, album, artist, playlist and library (v0.3.0)

## 0.2.0

- feat(player): queue, gapless playback and persistence (v0.2.0)
