# Journal des versions

Une entrée par version de `VERSION`. Une version **publiée** porte un tag `v<version>` :
c'est une release (installeur et APK en ligne), qui contient aussi les versions
intermédiaires. Les versions d'avant le 2026-11-10 viennent de l'ancienne ligne « Statut »
de `CLAUDE.md` ; celles qui n'y figuraient pas reprennent le sujet des commits qui ont posé leur numéro
(en anglais au début du projet).

Ce journal est aussi affiché dans l'app (Réglages › Application, #340). Une entrée propre à
une plateforme commence par `Android :`, `Windows :`, `Linux :` ou `Bureau :` (Windows et
Linux) et ne s'affiche que là ; sans préfixe, elle vaut pour toutes. Les entrées `chore:` ne
s'affichent pas.

## 0.5.70

- Blind test : depuis une playlist, un mix, un album, une liste intelligente ou les titres likés, devinez le titre ou l'artiste d'extraits de 15 s, en saisie libre ou parmi 4 propositions, avec score et série. Le titre reste masqué partout (écran verrouillé, Discord, notification) jusqu'à la réponse, et la file revient à la fin (#434).

## 0.5.69

- Transfert de la lecture et historique d'écoute aussi hors de chez vous : le téléphone joint l'ordinateur par le relais, chiffré de bout en bout (#430).

## 0.5.68

- Historique d'écoute synchronisé entre l'ordinateur et l'app Android liée, sur le réseau local : statistiques, rétrospective et « Écoutés dans Seaglass » comptent les écoutes des deux (#430).

## 0.5.67

- Minuteur de mise en veille : pause à la fin de l'album, de la playlist ou de la file (#436).

## 0.5.66

- Transfert de la lecture entre l'ordinateur et l'app Android : « Reprendre ici » et « Continuer sur l'ordinateur » depuis le téléphone, avec « Accepter les transferts » sur l'ordinateur (#414).

## 0.5.65

- Windows : la télécommande et la Jam sur le réseau local donnent l'adresse du réseau local, et non plus celle du VPN quand un VPN est actif (#425).

## 0.5.64

- Rétrospective du mois et de l'année, ouverte depuis les statistiques : temps d'écoute, jours d'écoute, titre et artiste de la période, découvertes, jour et heure d'écoute préférés, classements (#417).

## 0.5.63

- Fil d'actualité complété des sorties récentes des artistes suivis que TIDAL n'y met pas, avec un compteur des nouveautés pas encore vues dans la navigation (#415).

## 0.5.62

- Bureau : réglages propres à chaque sortie audio, désactivés par défaut (Réglages › Lecture › Sortie audio) : volume, normalisation et mode exclusif retenus par périphérique et rétablis quand il redevient la sortie, y compris derrière la sortie par défaut (#416).

## 0.5.61

- Bureau : raccourcis clavier globaux, désactivés par défaut (lecture/pause, suivant, précédent, volume, aimer le titre, afficher la fenêtre), combinaisons modifiables dans Réglages › Application ; une combinaison prise par une autre app est signalée (#413).

## 0.5.60

- Bureau : palette de commandes (Ctrl+K) pour aller à une page, lancer une action (lecture, minuteur, mode exclusif, thème, fenêtre…) ou chercher un titre, un album, un artiste ou une playlist ; `/` ouvre toujours la page Recherche (#412).

## 0.5.59

- Windows : le plein écran (F11) ne prend plus de retard au survol après quelques secondes (#191).

## 0.5.58

- Importer une playlist Deezer, Spotify ou Apple Music en collant son lien : aperçu des titres trouvés sur TIDAL, à vérifier ou introuvables, puis lecture ou création d'une playlist (#328).

## 0.5.57

- Dossiers de playlists : affichés dans Bibliothèque › Playlists et dans « Ajouter à une playlist », créés, renommés et supprimés depuis l'app, « Déplacer vers un dossier… » dans le menu d'une playlist ; rien ne change sans dossier (#331).

## 0.5.56

- Sélection multiple de titres : Ctrl+clic, Maj+clic et Ctrl+A sur le bureau, « Sélectionner » à l'appui long sur Android, puis lire ensuite, ajouter à la file ou à une playlist, liker ou retirer de la playlist en une fois (#332).

## 0.5.55

- Bureau : export de la bibliothèque (titres likés, albums, artistes, playlists) en JSON et CSV dans Documents › Seaglass, depuis Réglages › Application (#330).

## 0.5.54

- Clins d'œil, derniers détails : piste cachée dans la file le 29 février, minuteur en cadran le 21 octobre, barre de progression en horloge de gare le 1er août, et une recherche qui compte en parsecs (#399).

## 0.5.53

- Clins d'œil, fin : « En lecture » en format large sur une sélection de bandes originales de films, carton « Fin » quand la file s'achève avec le thème Séance, et refus d'une politesse inquiétante avec le thème Ordinateur de bord (#399).

## 0.5.52

- Clins d'œil, suite : volume à 11, ∞ sur la répétition après dix écoutes du même titre, générique de fin caché dans Réglages › Application, pochettes en gros pixels au clavier, piste introuvable dans une playlist vide, minuteur d'une heure, et deux détails propres aux thèmes Cyberpunk et Désert (#399).

## 0.5.51

- Clins d'œil : petits détails dans l'en-tête certains jours et effets brefs pour quelques recherches, sans son. Désactivables dans Réglages › Apparence (#399).

## 0.5.50

- Nouveau thème Générique : le papier découpé des génériques des années 50-60, aplats orange, rouges et crème sur papier noir, panneaux coupés aux ciseaux, titres de travers en étiquettes collées. La barre de progression est une ligne brisée et le chargement une spirale (#339).

## 0.5.49

- Nouveau thème Désert : space opera des sables, ocres, sable et ciel pâle, horizon bas, titres fins et très espacés, glyphes géométriques devant la navigation et un bleu profond réservé au bouton lecture. La progression laisse une trace dans le sable, le chargement est une ondulation de dune et, sur le bureau, un minuscule ver passe une fois dans l'horizon à la fin d'un album (#339).

## 0.5.48

- Nouveau thème Grand hôtel : comédie pastel et symétrique, rose poudré, bordeaux et moutarde, papier peint rayé, cadres doubles à filets, étiquettes en cartouche et numéros de piste en médaillons. Les sections de l'accueil sont numérotées en chapitres, la file s'intitule « Programme », une clé de concierge accompagne les réglages et « En lecture » est centré (#339).

## 0.5.47

- Nouveau thème Film noir : noir et blanc strict, ombres de stores vénitiens en biais, titres d'affiche des années 40 et informations à la machine à écrire ; seules les pochettes restent en couleur. Les notifications arrivent en télégrammes, une fumée immobile monte dans « En lecture » et la recherche sans résultat a son détective fatigué (#339).

## 0.5.46

- Nouveau thème Ordinateur de bord : les écrans de vaisseau de la science-fiction des années 70-80, ambre sur fond presque noir, étiquettes numérotées, blocs de couleur et bandes d'avertissement. La chaîne audio de « En lecture » devient un schéma de systèmes avec voyants, un voyant rouge fixe veille près du bouton lecture et le démarrage déroule une séquence d'auto-test (#339).

## 0.5.45

- Nouveau thème Séance : la salle de cinéma, noir de salle, rideaux de velours rouge, titres en capitales de générique. « En lecture » passe en format large avec le titre en carton-titre ; un repère de changement de bobine apparaît sur la pochette dans les 30 dernières secondes, le chargement est un décompte d'amorce et la chaîne audio une fiche technique de projection (#339).

## 0.5.44

- Nouveau thème Cassette : chaîne hi-fi des années 70, noyer, orange brûlé, bandes de jaquette. La barre de progression est une bande qui passe d'une bobine à l'autre, et la chaîne audio de « En lecture » s'affiche en vumètres à aiguille (#338).

## 0.5.43

- Nouveau thème Art déco : bleu nuit, filets dorés, éventails aux angles, titres en capitales géométriques, boutons en cartouches à pans coupés ; dans « En lecture », la pochette est encadrée comme une affiche (#338).

## 0.5.42

- Nouveau thème Estampe : gravure japonaise sur papier washi, encre indigo, titres en serif, cadres doubles, vagues et soleil en aplats, cachet vermillon (#338).

## 0.5.41

- Nouveau thème Minuit : noir pur, filets fins, aucun décor, la pochette est la seule couleur. Sur un écran OLED (comme celui de la plupart des téléphones), le noir éteint les pixels (#338).

## 0.5.40

- Page Apparence réorganisée : le thème actuel en haut, avec sa variante claire ou sombre et sa couleur d'accent, puis une galerie compacte en dernier, filtrable (tous, favoris, clairs, sombres). Une étoile met un thème en favori, en tête de la galerie (#390).

## 0.5.39

- Bureau : télécommande. Pilotez la lecture de l'ordinateur depuis le navigateur de votre téléphone, sur le même Wi-Fi : lecture / pause, titres précédent et suivant, position, volume, aléatoire, répétition, file d'attente, recherche, albums et playlists. À allumer dans Réglages › Application ; le QR code se scanne une fois, « Nouveau code » déconnecte les téléphones (#336).

## 0.5.38

- Playlists intelligentes calculées sur cet appareil : « Mes 50 du mois », « En boucle en ce moment », « Oubliés » et « Likés jamais écoutés ». On les ouvre depuis les statistiques, on peut les épingler et les ajouter à une playlist (#383).

## 0.5.37

- Nettoyer une de vos playlists ou vos titres likés : doublons (même titre, ou même chanson dans une autre édition) et titres retirés du catalogue, avec la même chanson dans une version disponible pour les remplacer ; on coche ce qu'on garde, on applique, et « Annuler » reste possible (#382).

## 0.5.36

- Bureau : mini-lecteur, une petite fenêtre avec la pochette, le titre, Précédent, Lecture / Pause, Suivant et le cœur, au premier plan au choix, qui remplace la grande fenêtre jusqu'à ce qu'on y revienne ; ouvert par le bouton de la barre de lecture, Ctrl+M ou la zone de notification, et rouvert à la même place (#384).

## 0.5.35

- Bloquer un titre ou un artiste depuis le menu d'un titre ou « En lecture » : il n'est plus proposé par les mixes ni par les radios, sur tous les appareils ; liste et « Débloquer » dans Réglages › Lecture (#334).

## 0.5.34

- Sauvegarde des réglages (Réglages › Application) : copier ses réglages d'interface et de lecture en texte, puis les coller dans Seaglass sur un autre appareil ; ce qui ne s'applique pas à l'appareil est ignoré et signalé (#352).

## 0.5.33

- Statistiques d'écoute (Bibliothèque › Statistiques) : titres, albums et artistes les plus écoutés et temps d'écoute sur 7 jours, 30 jours, 12 mois ou tout l'historique, calculés sur l'appareil ; l'historique garde maintenant 50 000 écoutes (#326).

## 0.5.32

- Page d'album : rangées « Autres versions » (deluxe, remaster, anniversaire…) et « Plus de <artiste> » en bas de page (#350).

## 0.5.31

- Windows : boutons Précédent, Lecture / Pause et Suivant sous la miniature de la fenêtre dans la barre des tâches, grisés quand l'action n'est pas possible et adaptés au thème clair ou sombre (#333).

## 0.5.30

- Android : une qualité en Wi-Fi et une autre en données mobiles, et un réglage qui baisse la qualité quand la batterie est faible (Haute sous 50 %, Basse sous 20 % ou avec l'économiseur de batterie, rien pendant la charge) (#365).

## 0.5.29

- Musique générée par IA : badge « IA » sur les titres et albums que le service marque ainsi, et réglage « Masquer la musique générée par IA », activé par défaut, avec la même portée que le filtre explicite (#369).

## 0.5.28

- Réglage « Masquer le contenu explicite » (Réglages › Lecture) : les titres et albums explicites disparaissent de la recherche, des pages, de l'accueil, des mixes, des radios et de la Jam ; vos favoris et vos playlists restent complets. Badge « E » sur les albums (#368).

## 0.5.27

- Android : deux qualités de plus dans Réglages › Lecture, Basse (AAC 96 kbit/s) et Max (FLAC jusqu'à 24 bits / 192 kHz, avec l'édition hi-res comme sur le bureau) (#366).

## 0.5.26

- Android : le widget d'écran d'accueil et les raccourcis d'épingles de l'icône, annoncés en 0.5.24 et 0.5.25, manquaient dans l'app publiée (elle avait été construite avec une ancienne copie de son code Android) ; ils y sont maintenant (#370).

## 0.5.25

- Android : widget d'écran d'accueil avec la pochette, le titre, l'artiste et les boutons Précédent, Lecture / Pause et Suivant ; un appui sur la pochette ouvre « En lecture » (#329).

## 0.5.24

- Android : un appui long sur l'icône de l'app propose les quatre premières épingles ; un appui lance leur lecture, et chacune peut être posée en icône sur l'écran d'accueil. Lien `seaglass://pin/<clé>` pour Modes et routines (#335).

## 0.5.23

- chore: plus d'avertissement « method `factor` is never used » à la compilation de l'app Android (#357).

## 0.5.22

- Android : réglage « Qualité » dans Réglages › Lecture : Lossless (comme avant) ou Haute (AAC 320 kbit/s), qui demande environ trois fois moins de données pour le même son en Bluetooth (#325).

## 0.5.21

- Android : écran éteint ou app en arrière-plan, l'interface ne recalcule plus la position de lecture plusieurs fois par seconde pour rien (#324).

## 0.5.20

- Android : la musique se charge par grandes rafales (jusqu'à 5 minutes d'avance) au lieu d'un filet continu, pour laisser le Wi-Fi et le réseau mobile se reposer entre deux et économiser la batterie (#323).

## 0.5.19

- Bouton « Annuler » après avoir retiré un titre d'une playlist : il revient à sa position (#349).

## 0.5.18

- Bouton « Annuler » pendant quelques secondes après avoir vidé la file, retiré un titre de la file, enlevé un favori ou retiré un élément de « Plus tard » (#349).

## 0.5.17

- Animations de l'interface réglables dans Réglages › Apparence : suivre le système, réduites ou complètes (#351).

## 0.5.16

- Bureau : la lecture se met en pause quand le casque ou la sortie audio utilisée est débranché, au lieu de continuer sur les haut-parleurs (réglable dans Réglages › Lecture › Sortie audio) (#348).
- Bureau : option « Reprendre la lecture au lancement » (Réglages › Lecture › Démarrage), jamais à l'ouverture de session ni après un plantage (#353).

## 0.5.15

- Android : les écouteurs reprennent la lecture après une pause de plus d'une minute, app en arrière-plan (la notification reste affichée 10 minutes après une pause) (#247).

## 0.5.14

- Android : la file d'attente de « En lecture » défile de nouveau (#342).

## 0.5.13

- Enregistrer la file d'attente comme playlist (nouvelle ou existante), titre en cours compris, dans l'ordre de lecture (#327).

## 0.5.12

- Journal des versions dans Réglages › Application, limité aux changements de la plateforme (#340).

## 0.5.11

- Taille de l'interface réglable sur le bureau (80 à 150 %) dans Réglages › Apparence, avec Ctrl + « + », Ctrl + « - » et Ctrl + 0 (#337).

## 0.5.10

- Journaux et rapports de plantage gardés en local (bureau et Android) : `seaglass.log`, panics Rust, exceptions Kotlin et sorties anormales signalées par Android ; Réglages › Application › Journaux pour copier le journal ou le dernier rapport, et toast au lancement qui suit un plantage (#192).

## 0.5.9

- Linux : mpv, lancé depuis l'AppImage, ne reçoit plus ses chemins de bibliothèques et de plugins (il chargeait 95 bibliothèques de l'AppImage au lieu de celles du système) (#320).

## 0.5.8

- Linux : mises à jour automatiques de l'AppImage (signée, annoncée dans le même `latest.json` que Windows), relance après installation ; désactivées hors d'une AppImage, message clair si son dossier n'est pas modifiable (#314).

## 0.5.7

- Lecteur : `loadfile` envoyé en arguments nommés, que le mpv 0.37 d'Ubuntu 24.04 comprend aussi ; il refusait toute reprise à une position et la normalisation du volume (#316).

## 0.5.6

- Linux : contrôles média du système par MPRIS (touches média, applet du bureau, `playerctl`, KDE Connect) : titre, artiste, album, pochette, position, lecture/pause, suivant, précédent, saut, aléatoire, répétition et volume (#312).

## 0.5.5

- Linux : sortie audio PipeWire, PulseAudio ou ALSA selon ce qui est disponible, mpv arrêté avec l'app même sur un plantage, mode exclusif masqué hors Windows ; AppImage avec GStreamer embarqué (sans lui, la vérification anti-robot de la connexion ne s'affiche pas), session gardée dans le Secret Service (GNOME Keyring, KWallet) ; connexion dans le navigateur du système, TIDAL bloquant le navigateur intégré de Linux ; testée dans WSL (#17).

## 0.5.4

- Mode exclusif : plus de grésillements ni de bouts répétés sur les interfaces USB qui réveillent le lecteur moins souvent que leur période annoncée (Focusrite) ; tampon exclusif de 50 ms (#308).

## 0.5.3

- Android : plus d'avertissement « unused import » sur `flush_blocking` à la compilation, la réexport n'existe plus que sur le bureau comme son seul appelant (#307).

## 0.5.2

- Android : en mode conduite, une tuile lit sa playlist ou ses titres likés dans l'ordre de leur page (tri choisi compris), et la bande › à droite de chaque tuile ouvre sa page (#193).

## 0.5.1

- Android : en déplaçant une section de l'accueil au doigt, la page ne descend plus sans fin ; la ligne reste dans la liste et le défilement s'arrête à son bout (#302).

## 0.5.0

- Windows : Mode exclusif WASAPI (Réglages › Lecture) : sortie ouverte pour Seaglass seul, bascule à chaud, normalisation suspendue, échec d'ouverture signalé sans repli en partagé ; la chaîne audio lit le format réellement envoyé au périphérique et n'annonce « Bit-perfect » qu'en exclusif. Fin de la phase 5 (#245).

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

- Android : Liens `https://tidal.com/…` touchés ouverts dans l'app sur Android, à activer dans Réglages (#203).

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

- Android : Jam depuis Android, en ligne seulement (#25, étape 4, en cours de vérification sur le S21).

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

- Android : En-tête des pages centré sur Android, Lecture / Aléatoire puis les autres boutons sur deux lignes, filtre pleine largeur et tri en un bouton (#182).

## 0.4.73

- Android : Nom de l'app aligné sur les boutons de l'en-tête, centré avec la version dessous sur Android (#183).

## 0.4.72

- Playlists et onglets de la bibliothèque gardés d'un lancement à l'autre, longues listes affichées par morceaux (#181).

## 0.4.71

- Bandeau « À la une » tournant, masquable et passable (#185), lien vers le dépôt et les issues dans À propos (#188), vérification des mises à jour du bureau sans attendre le téléchargement (#187), images de l'installeur (#194).

## 0.4.70 (publiée le 2026-11-05)

- Bouton Radio du titre dans « En lecture » (#179), artistes non cliquables dans les lignes sur Android (#180), bandeau « À la une » sans « Ouvrir l'album » et plus bas sur Android (#184), langue du système lue dans `java.util.Locale` sur Android (#186), « Se déconnecter » sous À propos en `--danger`, plus dans l'en-tête du bureau (#189), « En lecture » compacte sur Android, sans étiquette (#195).

## 0.4.69 (publiée le 2026-11-04)

- Android : Glisser un titre vers la droite l'ajoute à la file sur Android (#174).

## 0.4.68

- Android : Paroles et file en plein écran dans « En lecture » sur Android (#173).

## 0.4.67

- Thèmes Alliage clair et sombre (#175).

## 0.4.66 (publiée le 2026-11-04)

- Titres de la file cliquables, lecture depuis la file (#167).

## 0.4.65

- Plus de plantage « no available activity » quand on pilote la lecture après avoir balayé l'app, commandes du lecteur en JNI direct (#169).

## 0.4.64

- « Précédent » ne remplit plus la file manuelle de titres que l'utilisateur n'y a jamais mis (#166).

## 0.4.63

- Android : Notification d'Android cliquable (ouvre « En lecture ») avec boutons aléatoire et répétition (#163).

## 0.4.62

- Lecture / Aléatoire dans les titres likés (#161), plus de numéros dans les playlists et mixes sur Android (#162).

## 0.4.61

- Android : Aucun fond ni décor animé sur Android (#158).

## 0.4.60

- L'APK porte la version de `VERSION` au lieu de 1.0 (#157).

## 0.4.59

- Playlists rouvertes sans « Chargement… », revalidées par ETag (#152).

## 0.4.58 (publiée le 2026-11-04)

- Android : Mode recherche dans une playlist et les titres likés sur Android (#151).

## 0.4.57

- Audit de sécurité et de performances, surtout Android (#144 à #148, voir « Audit (0.4.57) »).

## 0.4.56

- Messages éphémères centrés au-dessus de la lecture, sur une surface pleine, visibles dans « En lecture » (#150).

## 0.4.55

- Android : Onglet « Titres likés » dans la navigation d'Android, réglages en haut à droite (#143).

## 0.4.54 (publiée le 2026-11-03)

- Windows : L'installeur fait relire l'icône à Windows (#139).

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

- Android : APK release signé et mises à jour sur Android (#24).

## 0.4.46

- Android : Mise en page tactile sur Android (#24).

## 0.4.45

- Android : Refresh token dans l'Android Keystore (#24).

## 0.4.44

- Android : Lecture Media3 et session média sur Android (#24).

## 0.4.43 (publiée le 2026-11-03)

- Android : Squelette Android, sans lecture, avec connexion (#24).

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

- Windows : Première version publiée : installeur NSIS signé, mpv embarqué, mises à jour automatiques.

## 0.4.3

- chore: release v0.4.3 (fixes #6, #8, #12, #27)

## 0.4.2

- chore: Gitea issues access for Claude sessions via gitea-mcp (v0.4.2)

## 0.4.1

- feat(ui): Acidwave and Surreal themes (v0.4.1)

## 0.4.0

- feat(ui): three themes, now-playing view and signal path (v0.4.0)

## 0.3.1

- Windows : feat(integrations): Windows media controls via SMTC (v0.3.1)

## 0.3.0

- feat(views): home, search, album, artist, playlist and library (v0.3.0)

## 0.2.0

- feat(player): queue, gapless playback and persistence (v0.2.0)
