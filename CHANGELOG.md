# Changelog

One entry per `VERSION`. A **published** version carries a `v<version>` tag:
it is a release (installer and APK online), which also includes the intermediate
versions. Versions from before 2026-11-10 come from the old "Status" line
of `CLAUDE.md`; those that were not there reuse the subject of the commits that set their number.
The log was written in French until 0.5.96 and translated to English in 0.5.97.

This log is also shown in the app (Settings › Application, #340). An entry specific to
a platform starts with `Android:`, `Windows:`, `Linux:` or `Desktop:` (Windows and
Linux) and is only shown there; without a prefix, it applies to all. `chore:` entries are not
shown. A published version's title ends with `(published <YYYY-MM-DD>)`.

## 0.5.123

- chore: the Jam and remote control backends, the remote's command parsing, the player's transport controls, the favorites and the playback and system settings commands move to their own files, with no change in behavior (#45).

## 0.5.122

- chore: the player actor, the TIDAL models and client, the phone's handoff link and two large test suites are split into smaller files, with no change in behavior; a few technical error details are now in English (#45).

## 0.5.121

- chore: the Settings page is split into one component per group of settings, and its strings into one file per tab, with no visible change (#45).

## 0.5.120

- Desktop: the remote's code and the handoff key now live in the system's credential store, like the TIDAL session, instead of the settings database; existing ones move there on their own (#61).

## 0.5.119

- Desktop: handing playback over, or syncing a long history, from the phone over weak Wi-Fi no longer fails after 5 seconds: a large transfer gets up to 20 seconds (#60).

## 0.5.118

- Desktop: "Link a phone" (Settings › App › Remote) opens 10 minutes during which a phone can learn how to reach the computer away from home, with a new relay key each time; outside that window the key is no longer handed out on the local network (#62).
- Android: the linked computer's texts say to press "Link a phone" on the computer to also reach it away from home (#62).

## 0.5.117

- One stream per account, as TIDAL's terms require: pressing Play pauses the account's other devices (official apps included), and when another device starts playing, Seaglass pauses and says so (#35).

## 0.5.116

- chore: the player state types the interface receives are now generated from the Rust ones, so the two can no longer drift apart (#44).

## 0.5.115

- Previous while paused now resumes playback, also when it goes back to the start of the track (#47).

## 0.5.114

- Each listen is now reported to TIDAL the way its official apps report theirs (play log), so that it can count for the artists (#34).

## 0.5.113

- When TIDAL answers "too many requests", every request now waits for the same pause (following `Retry-After`) instead of retrying on its own, and new releases of followed artists are read 3 artists at a time (#37).

## 0.5.112

- The sign-in screen and Settings › Application say that Seaglass signs in with the identifier of TIDAL's Android app, not authorized by TIDAL for other apps, with a link to the explanation (#38).

## 0.5.111

- The Jam panel says that a Jam is for private use: TIDAL's terms do not allow public or commercial playback (#39).

## 0.5.110

- Desktop: "Copy as image" in the Recap view copies a summary card of the period (listening time, plays, days, new artists and the top 5 tracks) in the active theme's colors (#16).

## 0.5.109

- Desktop: the soundtrack widescreen bars in "Now playing" no longer hide the end of the queue or lyrics: the content fits between them, the bars follow the window's shape and slide out when the soundtrack ends (#11).

## 0.5.108

- chore: "Try it in your browser" on the website, the app's interface and themes with an invented catalog, built from the latest release (#28).

## 0.5.107

- chore: `cookie` 0.18.3 (RUSTSEC-2026-0594, private cookie nonces can repeat after a fork), pulled in by Tauri.

## 0.5.106 (published 2026-11-22)

- The project moved to GitHub: "Source code" and "Report a bug" in Settings › Application now open github.com/BeBanana1/Seaglass.
- chore: `url` 2.5.9 (RUSTSEC-2026-0592, stack overflow on nested `blob:` URLs).

## 0.5.105

- Desktop: "Copy as image" in the Now playing menu: a square card with the cover, title, artists, album and the played format, in the theme's colors, copied to the clipboard (#518).

## 0.5.104

- The "Play" buttons (album, playlist, mix, library, recap, drive mode, remote control) now turn shuffle off and play in order; a track picked in a list keeps the current shuffle mode (#526).

## 0.5.103

- Recap: audio panel with the share of hi-res, lossless and lossy listens, the share and time in bit-perfect, and the formats played most; each listen now records the format it was played in, synced with the linked device (#524).

## 0.5.102

- Recap: plays over the period (by month or by day), a day × hour heatmap, and moments: longest listening streak, most replayed track in a day, first track of the period (#521).

## 0.5.101

- Project website at seaglass.bebanana.ch: home and download pages, linked from Settings › Application; updates now come from seaglass.bebanana.ch/updates (#517)

## 0.5.100

- The home customization page opens again when the same album appears in several sections (#514).

## 0.5.99

- Smoother blind test clip bar and ring: no more small roll-backs, and the playback bar benefits too (#501).

## 0.5.98

- chore: frontend formatted by Prettier (`pnpm format`, checked in CI), `.editorconfig` for the editors

## 0.5.97

- The version log in Settings › Application is now written in English, like the rest of the project's documentation

## 0.5.96

- The signal path shows how far the track is in memory ("in memory up to 2:40") instead of the lead over playback, which went up and down in jolts while filling (#498)

## 0.5.95

- A track started from a search in a playlist or the liked tracks plays the whole list from it (shuffled if shuffle is on), not just the results (#496)

## 0.5.94

- Android: Settings › Playback says when the battery is currently lowering the quality (battery saver, low battery), instead of making the setting look ignored (#492)

## 0.5.93

- Android: sound is back in Low quality on Bluetooth headphones; HE-AAC is no longer sent to the audio chip (offload), High quality's AAC still is (#492)

## 0.5.92

- Android: no more micro-dropouts on Bluetooth headphones; the playback thread no longer sleeps between two fills of the audio buffer, except in offload (#491)

## 0.5.91

- chore: project shareable without the author's server: updater public key built in from `SEAGLASS_UPDATER_PUBKEY`, no default address left in the release scripts or the relay (#489)

## 0.5.90

- Android: FLAC decoded by the phone's audio chip when it can do it without gaps between tracks (offload), to put less load on the CPU with the screen off; turned off when the equalizer is active (#474)

## 0.5.89

- Android: no more progress bar in the mini player; its animation made the app redraw continuously during playback (#480)

## 0.5.88

- Android: the player puts less load on the CPU during screen-off playback (ExoPlayer dynamic scheduling, #474)

## 0.5.87

- Android: mini player progress bar animated by the compositor, less CPU during foreground playback (#473)

## 0.5.86

- Android: the app no longer starts with the desktop layout when the first call to the backend fails (#472)

## 0.5.85

- Notifications for new releases from followed artists, off by default (Settings › Notifications); a click opens the album, or the feed if there are several (#435)

## 0.5.84

- Optional scrobbling to ListenBrainz: token to paste in Settings › Application, listens kept offline and sent later (#431)

## 0.5.83

- Clickable credits: a name opens the list of tracks the person is credited on, with their roles; "Credits" entry on the artist page (#429)

## 0.5.82

- Android: parametric equalizer, same profiles and AutoEQ import as on desktop (#467)

## 0.5.81

- Desktop: parametric equalizer (peaks and shelves, preamp), named profiles, AutoEQ profile import, response curve; remembered per audio output with the per-device settings (#428)

## 0.5.80

- Jam: guests can vote to skip the current track, beyond a number of votes chosen by the host (#433)

## 0.5.79

- Configurable Jam relay (address and key) in Settings › Application, switch to remove the Jam from the app, automatic updates can be turned off; server addresses read at build time (#452)

## 0.5.78

- Jam: guests can read the lyrics of the current track, synced when TIDAL provides them (#448)

## 0.5.77

- The progress bar shows as a light band the part of the track already in memory, and the signal path its lead ("+2:40 in memory", "track in memory", "next ready") (#453)

## 0.5.76

- Jam: the host can let guests set the volume, under a cap they choose (#449)

## 0.5.75

- Android: a handle on the right of queue tracks lets you move them with a finger; a long press on the track still opens its menu (#450)

## 0.5.74

- Blind test: the remaining-time ring waits for the clip to actually play instead of starting during loading then going back to the start (#445)

## 0.5.73

- Blind test: the choices in 4-choice mode show the title on 2 lines instead of cropping it, in the app and on the Jam guest page (#446)

## 0.5.72

- chore: build without warnings: Android release that failed on an apksigner warning, dead code on Android, split bundle, non-incremental Kotlin compilation (#443)

## 0.5.71

- Multiplayer blind test: Jam guests answer from their phone, live ranking, bonus for the fastest, the host plays or hosts (#441).

## 0.5.70

- Blind test: from a playlist, a mix, an album, a smart list or the liked tracks, guess the title or artist of 15 s clips, by typing or among 4 choices, with score and streak. The track stays hidden everywhere (lock screen, Discord, notification) until the answer, and the queue comes back at the end (#434).

## 0.5.69

- Playback handoff and listening history away from home too: the phone reaches the computer through the relay, end-to-end encrypted (#430).

## 0.5.68

- Listening history synced between the computer and the linked Android app, on the local network: stats, recap and "Played in Seaglass" count the listens of both (#430).

## 0.5.67

- Sleep timer: pause at the end of the album, the playlist or the queue (#436).

## 0.5.66

- Playback handoff between the computer and the Android app: "Resume here" and "Continue on the computer" from the phone, with "Accept handoffs" on the computer (#414).

## 0.5.65

- Windows: the remote and the local network Jam give the local network address, no longer the VPN's when a VPN is active (#425).

## 0.5.64

- Monthly and yearly recap, opened from the stats: listening time, listening days, track and artist of the period, discoveries, favorite listening day and hour, rankings (#417).

## 0.5.63

- Feed completed with recent releases from followed artists that TIDAL leaves out, with a counter of unseen new items in the navigation (#415).

## 0.5.62

- Desktop: settings specific to each audio output, off by default (Settings › Playback › Audio output): volume, normalization and exclusive mode remembered per device and restored when it becomes the output again, including behind the default output (#416).

## 0.5.61

- Desktop: global keyboard shortcuts, off by default (play/pause, next, previous, volume, like the track, show the window), combinations editable in Settings › Application; a combination taken by another app is reported (#413).

## 0.5.60

- Desktop: command palette (Ctrl+K) to go to a page, run an action (playback, sleep timer, exclusive mode, theme, window…) or search for a track, album, artist or playlist; `/` still opens the Search page (#412).

## 0.5.59

- Windows: full screen (F11) no longer lags on hover after a few seconds (#191).

## 0.5.58

- Import a Deezer, Spotify or Apple Music playlist by pasting its link: preview of the tracks found on TIDAL, to check or not found, then playback or creation of a playlist (#328).

## 0.5.57

- Playlist folders: shown in Library › Playlists and in "Add to a playlist", created, renamed and deleted from the app, "Move to a folder…" in a playlist's menu; nothing changes without folders (#331).

## 0.5.56

- Multiple track selection: Ctrl+click, Shift+click and Ctrl+A on desktop, "Select" on long press on Android, then play next, add to the queue or to a playlist, like or remove from the playlist in one go (#332).

## 0.5.55

- Desktop: library export (liked tracks, albums, artists, playlists) as JSON and CSV in Documents › Seaglass, from Settings › Application (#330).

## 0.5.54

- Easter eggs, last details: a hidden track in the queue on February 29, a dial sleep timer on October 21, a station clock progress bar on August 1, and a search that counts in parsecs (#399).

## 0.5.53

- Easter eggs, the end: "Now playing" in widescreen on a selection of film soundtracks, a "The End" card when the queue ends with the Showing theme, and unsettlingly polite refusals with the Ship Computer theme (#399).

## 0.5.52

- Easter eggs, continued: volume at 11, ∞ on repeat after ten listens of the same track, end credits hidden in Settings › Application, big-pixel covers from the keyboard, a track not found in an empty playlist, a one-hour sleep timer, and two details specific to the Cyberpunk and Desert themes (#399).

## 0.5.51

- Easter eggs: small details in the header on certain days and brief effects for a few searches, without sound. Can be turned off in Settings › Appearance (#399).

## 0.5.50

- New Title Sequence theme: the cut paper of 50s-60s title sequences, orange, red and cream flats on black paper, panels cut with scissors, crooked titles as stuck-on labels. The progress bar is a broken line and loading a spiral (#339).

## 0.5.49

- New Desert theme: sand space opera, ochres, sand and pale sky, low horizon, thin and widely spaced titles, geometric glyphs in front of the navigation and a deep blue reserved for the play button. Progress leaves a trail in the sand, loading is a dune ripple and, on desktop, a tiny worm crosses the horizon once at the end of an album (#339).

## 0.5.48

- New Grand Hotel theme: pastel and symmetrical comedy, powder pink, burgundy and mustard, striped wallpaper, double-ruled frames, cartouche labels and track numbers as medallions. Home sections are numbered as chapters, the queue is titled "Programme", a concierge key accompanies the settings and "Now playing" is centered (#339).

## 0.5.47

- New Film Noir theme: strict black and white, slanted venetian blind shadows, 40s poster titles and typewriter information; only the covers stay in color. Notifications arrive as telegrams, still smoke rises in "Now playing" and the empty search has its tired detective (#339).

## 0.5.46

- New Ship Computer theme: the spaceship screens of 70s-80s science fiction, amber on near-black, numbered labels, color blocks and warning stripes. The "Now playing" signal path becomes a systems diagram with indicator lights, a fixed red light watches near the play button and startup runs a self-test sequence (#339).

## 0.5.45

- New Showing theme: the movie theater, theater black, red velvet curtains, titles in credit capitals. "Now playing" switches to widescreen with the title as an intertitle; a reel change cue appears on the cover in the last 30 seconds, loading is a leader countdown and the signal path a projection technical sheet (#339).

## 0.5.44

- New Cassette theme: 70s hi-fi, walnut, burnt orange, J-card stripes. The progress bar is a tape going from one reel to the other, and the "Now playing" signal path shows as needle VU meters (#338).

## 0.5.43

- New Art Deco theme: night blue, gold rules, fans in the corners, geometric capital titles, buttons as cut-corner cartouches; in "Now playing", the cover is framed like a poster (#338).

## 0.5.42

- New Estampe theme: Japanese woodblock print on washi paper, indigo ink, serif titles, double frames, flat waves and sun, vermilion seal (#338).

## 0.5.41

- New Midnight theme: pure black, thin rules, no decoration, the cover is the only color. On an OLED screen (like most phones'), black turns the pixels off (#338).

## 0.5.40

- Appearance page reorganized: the current theme at the top, with its light or dark variant and its accent color, then a compact gallery last, filterable (all, favorites, light, dark). A star makes a theme a favorite, at the top of the gallery (#390).

## 0.5.39

- Desktop: remote control. Drive the computer's playback from your phone's browser, on the same Wi-Fi: play / pause, previous and next tracks, position, volume, shuffle, repeat, queue, search, albums and playlists. Turn it on in Settings › Application; the QR code is scanned once, "New code" disconnects the phones (#336).

## 0.5.38

- Smart playlists computed on this device: "My top 50 this month", "On repeat", "Forgotten favorites" and "Liked, never played". They open from the stats, can be pinned and added to a playlist (#383).

## 0.5.37

- Clean up one of your playlists or your liked tracks: duplicates (same track, or same song in another edition) and tracks removed from the catalog, with the same song in an available version to replace them; check what to keep, apply, and "Undo" stays possible (#382).

## 0.5.36

- Desktop: mini player, a small window with the cover, the title, Previous, Play / Pause, Next and the heart, optionally always on top, which replaces the large window until you go back to it; opened by the playback bar button, Ctrl+M or the notification area, and reopened at the same place (#384).

## 0.5.35

- Block a track or an artist from a track's menu or "Now playing": it is no longer offered by mixes or radios, on all devices; list and "Unblock" in Settings › Playback (#334).

## 0.5.34

- Settings backup (Settings › Application): copy your interface and playback settings as text, then paste them into Seaglass on another device; what does not apply to the device is ignored and reported (#352).

## 0.5.33

- Listening stats (Library › Stats): most played tracks, albums and artists and listening time over 7 days, 30 days, 12 months or the whole history, computed on the device; the history now keeps 50,000 listens (#326).

## 0.5.32

- Album page: "Other versions" (deluxe, remaster, anniversary…) and "More by <artist>" rows at the bottom of the page (#350).

## 0.5.31

- Windows: Previous, Play / Pause and Next buttons under the window's thumbnail in the taskbar, grayed out when the action is not possible and adapted to the light or dark theme (#333).

## 0.5.30

- Android: one quality on Wi-Fi and another on mobile data, and a setting that lowers the quality when the battery is low (High under 50 %, Low under 20 % or with battery saver, nothing while charging) (#365).

## 0.5.29

- AI-generated music: "AI" badge on tracks and albums the service tags as such, and a "Hide AI-generated music" setting, on by default, with the same scope as the explicit filter (#369).

## 0.5.28

- "Hide explicit content" setting (Settings › Playback): explicit tracks and albums disappear from search, pages, home, mixes, radios and the Jam; your favorites and playlists stay complete. "E" badge on albums (#368).

## 0.5.27

- Android: two more qualities in Settings › Playback, Low (AAC 96 kbit/s) and Max (FLAC up to 24-bit / 192 kHz, with the hi-res edition as on desktop) (#366).

## 0.5.26

- Android: the home screen widget and the app icon's pin shortcuts, announced in 0.5.24 and 0.5.25, were missing from the published app (it had been built with an old copy of its Android code); they are there now (#370).

## 0.5.25

- Android: home screen widget with the cover, title, artist and the Previous, Play / Pause and Next buttons; tapping the cover opens "Now playing" (#329).

## 0.5.24

- Android: a long press on the app icon offers the first four pins; a tap starts their playback, and each can be placed as an icon on the home screen. Link `seaglass://pin/<key>` for Modes and Routines (#335).

## 0.5.23

- chore: no more "method `factor` is never used" warning when building the Android app (#357).

## 0.5.22

- Android: "Quality" setting in Settings › Playback: Lossless (as before) or High (AAC 320 kbit/s), which needs about three times less data for the same sound over Bluetooth (#325).

## 0.5.21

- Android: with the screen off or the app in the background, the interface no longer recomputes the playback position several times per second for nothing (#324).

## 0.5.20

- Android: music loads in large bursts (up to 5 minutes ahead) instead of a continuous trickle, to let Wi-Fi and the mobile network rest in between and save battery (#323).

## 0.5.19

- "Undo" button after removing a track from a playlist: it goes back to its position (#349).

## 0.5.18

- "Undo" button for a few seconds after clearing the queue, removing a track from the queue, removing a favorite or removing an item from "Save for later" (#349).

## 0.5.17

- Interface animations configurable in Settings › Appearance: follow the system, reduced or full (#351).

## 0.5.16

- Desktop: playback pauses when the headphones or the audio output in use are unplugged, instead of continuing on the speakers (configurable in Settings › Playback › Audio output) (#348).
- Desktop: "Resume playback at launch" option (Settings › Playback › Startup), never at sign-in or after a crash (#353).

## 0.5.15

- Android: earbuds resume playback after a pause of more than a minute, app in the background (the notification stays shown 10 minutes after a pause) (#247).

## 0.5.14

- Android: the "Now playing" queue scrolls again (#342).

## 0.5.13

- Save the queue as a playlist (new or existing), current track included, in play order (#327).

## 0.5.12

- Version log in Settings › Application, limited to the platform's changes (#340).

## 0.5.11

- Interface size configurable on desktop (80 to 150 %) in Settings › Appearance, with Ctrl + "+", Ctrl + "-" and Ctrl + 0 (#337).

## 0.5.10

- Logs and crash reports kept locally (desktop and Android): `seaglass.log`, Rust panics, Kotlin exceptions and abnormal exits reported by Android; Settings › Application › Logs to copy the log or the last report, and a toast at the launch following a crash (#192).

## 0.5.9

- Linux: mpv, launched from the AppImage, no longer receives its library and plugin paths (it loaded 95 libraries from the AppImage instead of the system's) (#320).

## 0.5.8

- Linux: automatic updates of the AppImage (signed, announced in the same `latest.json` as Windows), relaunch after installation; disabled outside an AppImage, clear message if its folder is not writable (#314).

## 0.5.7

- Player: `loadfile` sent with named arguments, which Ubuntu 24.04's mpv 0.37 also understands; it refused any resume at a position and volume normalization (#316).

## 0.5.6

- Linux: system media controls through MPRIS (media keys, desktop applet, `playerctl`, KDE Connect): title, artist, album, cover, position, play/pause, next, previous, seek, shuffle, repeat and volume (#312).

## 0.5.5

- Linux: PipeWire, PulseAudio or ALSA audio output depending on what is available, mpv stopped with the app even on a crash, exclusive mode hidden outside Windows; AppImage with bundled GStreamer (without it, the sign-in bot check does not show), session kept in the Secret Service (GNOME Keyring, KWallet); sign-in in the system browser, TIDAL blocking Linux's built-in browser; tested in WSL (#17).

## 0.5.4

- Exclusive mode: no more crackles or repeated bits on USB interfaces that wake the player less often than their announced period (Focusrite); 50 ms exclusive buffer (#308).

## 0.5.3

- Android: no more "unused import" warning on `flush_blocking` when building, the re-export now only exists on desktop like its only caller (#307).

## 0.5.2

- Android: in driving mode, a tile plays its playlist or liked tracks in the order of their page (chosen sort included), and the › band on the right of each tile opens its page (#193).

## 0.5.1

- Android: when moving a home section with a finger, the page no longer scrolls down endlessly; the row stays in the list and scrolling stops at its end (#302).

## 0.5.0

- Windows: WASAPI exclusive mode (Settings › Playback): output opened for Seaglass alone, live toggle, normalization suspended, opening failure reported without falling back to shared mode; the signal path reads the format actually sent to the device and only announces "Bit-perfect" in exclusive mode. End of phase 5 (#245).

## 0.4.119

- Android: Back closes driving mode opened by the shortcut or the link instead of quitting the app; Back is now asked directly of the interface (#193).

## 0.4.118

- Android: full-screen driving mode (large playback buttons, 1 to 8 tiles that start a playlist, a mix, the liked tracks or "Save for later" in one tap), opened by the car icon, the app shortcut or the `seaglass://drive` link; mixes can also be pinned in the desktop sidebar (#193).

## 0.4.117

- Android: repeat and shuffle requested by a controller (car stereo, watch) go back to Rust, and the media session no longer offers to change ExoPlayer's playlist, speed or volume (#254).

## 0.4.116

- Jam: limits counted per guest (cap 5 times wider per address), failed code attempts forgotten after a quarter hour, 100 Jam tracks pending at most, code remembered per Jam in the page; relay to redeploy (#256).

## 0.4.115

- Frontend tests with `vitest` (list sort and filter, formats, signal path, links, reordering, swipe), run by CI; nothing changes in the app (#264).

## 0.4.114

- Android: the current track is highlighted as soon as it starts, no longer only after a pause (#291).

## 0.4.113

- Customized home: drag and drop of sections works again on Windows (the queue and playlists too), and on Android a section moves with a long press then dragging the finger (#290).

## 0.4.112

- Jam: host controls (collapsed): per-guest limit on pending tracks, albums allowed or not, additions paused, guests kicked or readmitted; a track already in the queue is refused (#288).

## 0.4.111

- Frozen mpv: stopped after 5 s without an answer instead of blocking the player; stopped or crashed mpv: restarted once and the track resumes (#255).

## 0.4.110

- "Now playing" shows the hi-res edition actually played, with its format, and opens its album (#111).

## 0.4.109

- Session: a transient TIDAL outage no longer erases the refresh token, a session not resumed at launch is retried on its own, and a session refused midway goes back to the sign-in screen (#252, #259).

## 0.4.108

- Android: Back only quits the app from the home page, and "Now playing" opened from the notification goes back to the page the track plays from (#240).

## 0.4.107

- Current track more visible in lists: tinted background, accent bar and equalizer (#241).

## 0.4.106

- Home sections can be reordered by drag and drop in "Customize" (#246).

## 0.4.105

- Search runs as you type, the history only keeps the kept query (#248).

## 0.4.104

- Accent color kept for each theme (#250).

## 0.4.103

- "Save for later" can be pinned in the sidebar (#244).

## 0.4.102

- "Save for later": bookmark next to the heart to remove a track in one click (#249).

## 0.4.101

- Sign-in in private browsing, browsing data cleared on sign-out (#258).

## 0.4.100

- Android: toasts above the open keyboard (#251).

## 0.4.99

- Android: lyrics (and queue) panel centered in full screen (#242).

## 0.4.98

- Android: display locked in portrait (#243).

## 0.4.97

- Small defects from the audit: guest page fragment, HTTPS-only HTTP clients, "feat." cut, `FileProvider` paths (#262).

## 0.4.96

- Last save of the queue, position and volume on shutdown and before an update (#260).

## 0.4.95

- mpv from `PATH` used only in debug; "mpv not found" banner if the bundled mpv is missing (#261).

## 0.4.94 (published 2026-11-06)

- Buttons with music app logos on the Jam guest page (#225).

## 0.4.93 (published 2026-11-06)

- Android: tapped `https://tidal.com/…` links opened in the app on Android, to turn on in Settings (#203).

## 0.4.92 (published 2026-11-06)

- Discord presence, "Listening to Seaglass" (#215).

## 0.4.91

- Spotify, Deezer, Apple Music, YouTube Music or TIDAL link pasted into the Jam search (#229), links to these apps on the guest page (#225).

## 0.4.90

- Search tracks whose name matches the query come before those of artists with that name, in the app and the Jam (#232).

## 0.4.89

- Clicking the notification area icon hides the window already in front (#226), "Save for later" button in an album's header (#227).

## 0.4.88

- Albums in the Jam, and removal of their own additions by a guest (#223).

## 0.4.87

- Play / pause by Jam guests, at the host's choice (#222).

## 0.4.86

- Cross to clear the Jam search (#224).

## 0.4.85 (published 2026-11-06)

- Quick access to the Jam from "Now playing", relay resume delay raised to 30 min (#25).

## 0.4.84

- Android: Jam from Android, online only (#25, step 4, being verified on the S21).

## 0.4.83

- Online Jam through the relay, without shared Wi-Fi (#25, step 3b), local network as second choice.

## 0.4.82

- Jam relay (#25, step 3a, `relay/`, to deploy), guest page with relative paths.

## 0.4.81

- Local Jam, step 2 (#25): QR code, guest's first name in the queue, state pushed live to guests.

## 0.4.80

- "Save for later" (TIDAL's Save for later), library tab and menu entry (#196).

## 0.4.79

- Local Jam, step 1 (#25): guests on the same network add tracks to the queue from a web page.

## 0.4.78 (published 2026-11-05)

- Manage your playlists, reorder, rename, delete (#201), local listening history and "Played in Seaglass" (#202), shared link to the app on Android (#203, to verify on the S21), mixes in the library (#204), full artist page (#205).

## 0.4.77

- Full home page, next pages of the feed (#207).

## 0.4.76

- Search history (#211).

## 0.4.75

- Pasted TIDAL link opened in the app, on desktop (#203), credits of a whole album (#206).

## 0.4.74

- Android: page headers centered on Android, Play / Shuffle then the other buttons on two rows, full-width filter and sort in one button (#182).

## 0.4.73

- Android: app name aligned with the header buttons, centered with the version below on Android (#183).

## 0.4.72

- Playlists and library tabs kept from one launch to the next, long lists shown in chunks (#181).

## 0.4.71

- Rotating "Featured" banner, which can be hidden and skipped (#185), link to the repository and issues in About (#188), desktop update check without waiting for the download (#187), installer images (#194).

## 0.4.70 (published 2026-11-05)

- Track Radio button in "Now playing" (#179), artists not clickable in rows on Android (#180), "Featured" banner without "Open album" and lower on Android (#184), system language read from `java.util.Locale` on Android (#186), "Sign out" under About in `--danger`, no longer in the desktop header (#189), compact "Now playing" on Android, without a label (#195).

## 0.4.69 (published 2026-11-04)

- Android: swiping a track to the right adds it to the queue on Android (#174).

## 0.4.68

- Android: lyrics and queue in full screen in "Now playing" on Android (#173).

## 0.4.67

- Light and dark Alloy themes (#175).

## 0.4.66 (published 2026-11-04)

- Clickable queue tracks, playback from the queue (#167).

## 0.4.65

- No more "no available activity" crash when controlling playback after swiping the app away, player commands through direct JNI (#169).

## 0.4.64

- "Previous" no longer fills the manual queue with tracks the user never put there (#166).

## 0.4.63

- Android: clickable Android notification (opens "Now playing") with shuffle and repeat buttons (#163).

## 0.4.62

- Play / Shuffle in liked tracks (#161), no more numbers in playlists and mixes on Android (#162).

## 0.4.61

- Android: no animated background or decoration on Android (#158).

## 0.4.60

- The APK carries the version from `VERSION` instead of 1.0 (#157).

## 0.4.59

- Playlists reopened without "Loading…", revalidated through ETag (#152).

## 0.4.58 (published 2026-11-04)

- Android: search mode in a playlist and the liked tracks on Android (#151).

## 0.4.57

- Security and performance audit, mostly Android (#144 to #148, see "Audit (0.4.57)").

## 0.4.56

- Short-lived messages centered above playback, on a solid surface, visible in "Now playing" (#150).

## 0.4.55

- Android: "Liked tracks" tab in Android's navigation, settings at the top right (#143).

## 0.4.54 (published 2026-11-03)

- Windows: the installer makes Windows reread the icon (#139).

## 0.4.53

- Faster release builds (ThinLTO, no more `staticlib`), Android build warnings handled.

## 0.4.52 (published 2026-11-03)

- New logo, vinyl with sun and sea, on Android too (#126).

## 0.4.51

- Real full screen (F11) (#131).

## 0.4.49

- "Playing from" in "Now playing" (#124).

## 0.4.48

- Lyrics and queue side by side in "Now playing" (#119).

## 0.4.47 (published 2026-11-03)

- Android: signed release APK and updates on Android (#24).

## 0.4.46

- Android: touch layout on Android (#24).

## 0.4.45

- Android: refresh token in the Android Keystore (#24).

## 0.4.44

- Android: Media3 playback and media session on Android (#24).

## 0.4.43 (published 2026-11-03)

- Android: Android skeleton, without playback, with sign-in (#24).

## 0.4.42

- Abstract playback engine, first step of the Android app (#24).

## 0.4.41 (published 2026-11-03)

- (no note in the old status)

## 0.4.40

- feat(integrations): notification area icon and close to tray (#83)

## 0.4.37

- feat(library): sort remembered per playlist and for liked tracks (#93)

## 0.4.36

- perf(ui): list thumbnails at 160 px (#89)

## 0.4.35

- Aero theme (#101).

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

- feat(player): clear message when the CPU lacks the AVX2 required by mpv (#92)
- feat(player): choice of the audio output device (#79)
- feat(library): album, artist and playlist favorites (#81)
- feat(player): sleep timer (#85)
- perf(ui): long lists without recomputing on every position (#88)
- feat(info): track credits in Track info (#84)
- feat(player): volume normalization through ReplayGain (#82)

## 0.4.24 (published 2026-10-30)

- Bauhaus theme (#70).

## 0.4.23

- Risograph theme (#69).

## 0.4.22

- Terminal theme (#68).

## 0.4.21

- Light and dark Neumorphism themes (#67).

## 0.4.20

- Cyberpunk theme (#66).

## 0.4.19

- Solarpunk theme (#65).

## 0.4.18

- Theme tokens, prerequisite of the #62 theme batch (#64).

## 0.4.17

- Interface in English (#7).

## 0.4.16

- Mix and radio images (#56).

## 0.4.15 (published 2026-10-30)

- Hi-res edition in "Ask" mode and link to the hi-res album (#59).

## 0.4.14 (published 2026-10-30)

- Track info and bitrate in the signal path (#3).

## 0.4.13

- Hi-res edition also found on a twin album with different ISRCs (#55).

## 0.4.12 (published 2026-10-30)

- Liked tracks and adding to playlists (#4).

## 0.4.11

- Track context menu (#52).

## 0.4.10 (published 2026-10-30)

- Hi-res edition shown clearly in the signal path (#18).

## 0.4.9

- Monthly mixes in the feed (#14).

## 0.4.8

- Mixes and radios (#11), feed (#14), synced lyrics (#5), hi-res edition of a track that has none (#18).

## 0.4.7 (published 2026-10-30)

- Installer in French, download page at `https://updates.bebanana.ch/seaglass/`.

## 0.4.6 (published 2026-10-30)

- Pinned playlists (#10), accent color (#23), clear the queue (#41), "Check for updates" button (#43).

## 0.4.5 (published 2026-10-30)

- Settings page (#9), volume (#29), mpv cache, launch at startup (#13), customizable home (#21, #22), list filter and sort (#1, #2), adding albums to the queue (#15), queue in "Now playing" (#20).

## 0.4.4 (published 2026-10-29)

- Windows: first published version: signed NSIS installer, bundled mpv, automatic updates.

## 0.4.3

- chore: release v0.4.3 (fixes #6, #8, #12, #27)

## 0.4.2

- chore: Gitea issues access for Claude sessions via gitea-mcp (v0.4.2)

## 0.4.1

- feat(ui): Acidwave and Surreal themes (v0.4.1)

## 0.4.0

- feat(ui): three themes, now-playing view and signal path (v0.4.0)

## 0.3.1

- Windows: feat(integrations): Windows media controls via SMTC (v0.3.1)

## 0.3.0

- feat(views): home, search, album, artist, playlist and library (v0.3.0)

## 0.2.0

- feat(player): queue, gapless playback and persistence (v0.2.0)
