# Audits

Notes d'implémentation de Seaglass, lues à la demande (index dans [`CLAUDE.md`](../CLAUDE.md)).

## Audit du 2026-11-10 (0.4.94)

Audit de tout le projet par lecture du code, avec `fmt`, `clippy`, les tests et `pnpm check`. Rien de grave. Suites dans les issues #253 à #265 : reprise du relais de la Jam (#253), commandes de la session média d'Android (#254), mpv figé ou planté (#255), limites de la Jam (#256, #257), cookies de la page de connexion (#258), session refusée en cours de route (#259), sauvegarde à la fermeture (#260), mpv du `PATH` (#261), petits défauts (#262), `cargo audit` en CI (#263), tests du frontend (#264), ce découpage de la documentation (#265). Vérifié à cette occasion dans le code de Tauri 2.12.5 : une origine distante (fenêtre de connexion) passe toujours par l'ACL, même pour les commandes de l'app. Non couvert : `cargo audit` (outil absent), essais sur appareil, code Windows `unsafe` en détail.

Petits défauts (#262, 0.4.97) : fragment illisible ignoré par la page des invités ; `https_only(true)` sur les clients TIDAL, liens d'autres services et APK (le test de `tidal/mod.rs` qui vise `127.0.0.1` en HTTP a son propre client) ; `strip_decorations` cherche « feat. » avec `to_ascii_lowercase`. Le `FileProvider` d'Android est **gardé** : wry (`RustWebChromeClient.createImageFile`) s'en sert pour une photo prise depuis un `<input type="file">` ; ses chemins sont réduits à `getExternalFilesDir(Pictures)` (`res/xml/file_paths.xml`).

## Audit (0.4.57, #144 à #148)

Audit de sécurité et de performances du 2026-11-03, par lecture du code. Rien de grave côté sécurité (CSP stricte, capability `core:default`, plugin mobile sans commande pour l'UI, jeton du serveur de manifestes, Keystore). Corrigé : URLs signées dans les erreurs réseau (#144), page de connexion Android et sauvegarde (#145), rendu Android (#146), position et sauvegarde en arrière-plan (#147), rafraîchissement du token et résolution (#148). Écarté, avec la raison dans l'issue : filtrer les contrôleurs de la session média (#145, risque pour les Buds et l'écran verrouillé), compression HTTP (#148, TIDAL ne compresse pas). Non traité : virtualisation des longues listes sur Android (à mesurer sur le S21 d'abord).
