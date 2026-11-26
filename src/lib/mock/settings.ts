import { emit, fx, params, type Handlers } from "./core";
import type { Lang, LanguagePref, Theme, TrackSort, UiSettings } from "../ipc";

let pinShortcutParam = params.has("pinshortcut");

export let ui: UiSettings = {
  theme: (params.get("theme") as Theme) ?? "seaglass",
  home: { order: [], hidden: [], featuredHidden: false },
  // La playlist des fixtures et les titres likés, pour voir la section « Épinglés ».
  pins: [
    { kind: "playlist", uuid: fx.playlist.playlist.uuid, title: fx.playlist.playlist.title },
    { kind: "likedTracks" },
  ],
  // `?accent=ff5c8a` : couleur d'accent personnalisée du thème affiché.
  accents: params.get("accent")
    ? { [(params.get("theme") as Theme) ?? "seaglass"]: `#${params.get("accent")}` }
    : {},
  // `?favorites=alloy,terminal` : thèmes favoris (#390).
  favoriteThemes: (params.get("favorites")?.split(",") ?? []) as Theme[],
  // `?lang=en` : interface en anglais.
  language: (params.get("lang") as LanguagePref) ?? "system",
  // `?tray=1` : fermer la fenêtre la réduit dans la zone de notification (#83).
  closeAction: params.has("tray") ? "tray" : "quit",
  // `?discord=1` : présence Discord activée (#215).
  discord: params.has("discord"),
  sorts: [],
  zoom: 100,
  // `?motion=reduced|full` : animations réduites ou forcées (#351).
  motion: (params.get("motion") as UiSettings["motion"]) ?? "system",
  // `?eggs=off` : clins d'œil désactivés (#399) ; `?date=` et `?egg=` dans `lib/eggs/`.
  easterEggs: params.get("eggs") !== "off",
  // `?explicit=hide` : contenu explicite masqué (#368) ; `?ai=show` : musique générée par
  // IA affichée (#369). Le backend factice ne filtre pas.
  content: { hideExplicit: params.get("explicit") === "hide", hideAi: params.get("ai") !== "show" },
  // `?hotkeys=1` : raccourcis globaux activés (#413) ; `?hotkeys=taken` : le premier est pris.
  globalShortcuts: {
    enabled: params.has("hotkeys"),
    bindings: {
      playPause: "Ctrl+Alt+Space",
      next: "Ctrl+Alt+ArrowRight",
      previous: "Ctrl+Alt+ArrowLeft",
      volumeUp: "Ctrl+Alt+ArrowUp",
      volumeDown: "Ctrl+Alt+ArrowDown",
      like: "Ctrl+Alt+KeyL",
      showWindow: "Ctrl+Alt+KeyS",
    },
  },
  // `?autoupdates=off` : vérification périodique coupée ; `?jam=off` : Jam masquée (#452).
  autoUpdates: params.get("autoupdates") !== "off",
  jam: params.get("jam") !== "off",
  releaseNotifications: false,
};

/** Website demo (#28): the appearance choices are kept between visits, in the browser. */
const DEMO_UI_KEY = "seaglass-demo-ui";
const DEMO_UI_FIELDS = [
  "theme",
  "accents",
  "favoriteThemes",
  "language",
  "motion",
  "zoom",
] as const;
if (__DEMO__) {
  try {
    const saved = JSON.parse(localStorage.getItem(DEMO_UI_KEY) ?? "{}") as Partial<UiSettings>;
    for (const field of DEMO_UI_FIELDS) {
      if (saved[field] !== undefined && !params.has(field)) ui = { ...ui, [field]: saved[field] };
    }
  } catch {
    // Storage blocked or unreadable: the defaults stay.
  }
}
export function saveDemoUi() {
  try {
    const kept = Object.fromEntries(DEMO_UI_FIELDS.map((field) => [field, ui[field]]));
    localStorage.setItem(DEMO_UI_KEY, JSON.stringify(kept));
  } catch {
    // Storage blocked: the choices only last for the visit.
  }
}

/** « Système » du backend factice : la langue du navigateur. */
export function effectiveLanguage(pref: LanguagePref): Lang {
  if (pref !== "system") return pref;
  return navigator.language.startsWith("fr") ? "fr" : "en";
}

let autostart = params.has("autostart");
let fullscreen = params.has("fullscreen");
let miniOnTop = true;

let mockDownloaded = params.get("update") !== "download";

function mockUpdate() {
  return params.has("update")
    ? {
        version: "0.5.0",
        currentVersion: "0.4.4",
        notes: ["- Installeur Windows", "- Mises à jour automatiques"].join("\n"),
        // Android : proposée avant d'être téléchargée, avec la taille de l'APK.
        ...(params.get("platform") === "android" ? { size: 41_200_000 } : {}),
      }
    : null;
}

let notificationTaken = false;

export const settingsHandlers: Handlers = {
  // `?updates=none` : app compilée sans serveur de mises à jour (#452).
  update_server: () =>
    params.get("updates") === "none" ? null : "https://updates.example.org/seaglass",
  ui_settings: () => ui,
  // `?update=1` : simule une mise à jour prête.
  update_status: () => (mockDownloaded ? mockUpdate() : null),
  install_update: () => null,
  // `?updates=manual` : Linux hors d'une AppImage, pas de mise à jour automatique (#314).
  updates_available: () => params.get("updates") !== "manual",
  // Sans `?update=1`, l'app est « à jour ». `?update=download` : comme le bureau (#187),
  // la réponse arrive avant l'installeur, `update-ready` 2 s plus tard.
  check_for_updates: () => {
    const u = mockUpdate();
    if (u && !mockDownloaded) {
      setTimeout(() => {
        mockDownloaded = true;
        emit("update-ready", u);
      }, 2000);
      return { ...u, downloading: true };
    }
    if (u) emit("update-ready", u);
    return u;
  },
  set_ui_settings: ({ settings }) => {
    // Proxy Svelte : copie en JSON (voir les épingles dans docs/interface.md).
    ui = JSON.parse(JSON.stringify(settings)) as UiSettings;
  },
  language: () => effectiveLanguage(ui.language),
  set_language: ({ language }) => {
    ui.language = language as LanguagePref;
    return effectiveLanguage(ui.language);
  },
  set_home_layout: ({ sections }) => {
    const list = sections as { key: string; hidden: boolean }[];
    ui.home = {
      ...ui.home,
      order: list.map((s) => s.key),
      hidden: list.filter((s) => s.hidden).map((s) => s.key),
    };
    return ui.home;
  },
  set_home_featured: ({ shown }) => (ui.home = { ...ui.home, featuredHidden: !shown }),
  open_project_page: ({ page }) => {
    // Website demo (#28): the real links, in a new tab.
    if (__DEMO__) {
      const repo = "https://github.com/BeBanana1/Seaglass";
      const url = {
        website: "https://seaglass.bebanana.ch/",
        repo,
        newIssue: `${repo}/issues/new/choose`,
      }[String(page)];
      if (url) window.open(url, "_blank", "noopener");
    } else console.info("open_project_page", page);
    return null;
  },
  // #192 : `?crash=1` simule un plantage au lancement précédent (toast et rapport).
  take_crash_notice: () => params.get("crash") === "1",
  last_crash_report: () =>
    params.get("crash") === "1"
      ? "# Rapport de plantage Seaglass\n\npanic dans le thread « main » : exemple\n"
      : null,
  changelog: () => [
    {
      version: "0.5.12",
      published: null,
      entries: ["Journal des versions dans Réglages › Application (#340)."],
    },
    {
      version: "0.5.11",
      published: "2026-11-11",
      entries: ["Taille de l'interface réglable avec `Ctrl` + « + » (#337)."],
    },
  ],
  current_log: () => "2026-11-11T10:00:00Z  INFO seaglass_lib: Seaglass (aperçu)\n",
  open_log_dir: () => null,
  // #330 : progression simulée sur trois playlists.
  export_library: async () => {
    for (let done = 0; done <= 3; done++) {
      emit("library-export", { done, total: 3 });
      await new Promise((r) => setTimeout(r, 400));
    }
    return {
      dir: "C:\\Users\\aperçu\\Documents\\Seaglass\\export-2026-11-12-182600",
      tracks: 120,
      albums: 14,
      artists: 9,
      playlists: 3,
      failed: [],
      truncated: false,
    };
  },
  cancel_library_export: () => null,
  export_settings: () => JSON.stringify({ format: "seaglass-settings", version: 1, ui }, null, 2),
  import_settings: () => ({ applied: ["ui.theme"], ignored: ["playback.wifiQuality"] }),
  reset_home_layout: () => (ui.home = { order: [], hidden: [], featuredHidden: false }),
  // Copie JSON : l'UI envoie des proxys Svelte, que `structuredClone` refuse.
  set_pins: ({ pins }) => (ui.pins = JSON.parse(JSON.stringify(pins))),
  // Comme le backend : le plus récent en tête, le tri par défaut de la liste est oublié.
  set_list_sort: ({ list, sort }) => {
    const id = list as string;
    const next = sort as TrackSort | null;
    const fallback = id === "likedTracks" ? "added:true" : "position:false";
    ui.sorts = (ui.sorts ?? []).filter((s) => s.list !== id);
    if (next && `${next.key}:${next.descending}` !== fallback) {
      ui.sorts.unshift({ list: id, key: next.key, descending: next.descending });
    }
    return ui.sorts;
  },
  // `?platform=android` : mise en page tactile.
  // Website demo (#28): a phone gets the touch layout, as the Android app would show it.
  platform: () =>
    params.get("platform") === "android" ||
    (__DEMO__ && !params.has("platform") && matchMedia("(max-width: 700px)").matches)
      ? "android"
      : "desktop",
  autostart_enabled: () => autostart,
  discord_available: () => true,
  exclusive_available: () => true,
  global_shortcuts_status: () => ({
    available: true,
    failed: ui.globalShortcuts.enabled && params.get("hotkeys") === "taken" ? ["playPause"] : [],
  }),
  // `?notification=1` : appui sur la notification d'Android au lancement (#163, #240), une fois.
  take_now_playing_request: () => {
    const asked = params.get("notification") === "1" && !notificationTaken;
    notificationTaken = true;
    return asked;
  },
  take_shared_link: () => null,
  // Mode conduite (#193) : `?platform=android&drive=1` l'ouvre au lancement (`initialView`).
  take_drive_request: () => false,
  // Raccourci d'une épingle (#335) : `?platform=android&pinshortcut=1` lit la première épingle.
  take_pin_request: () => {
    const pin = pinShortcutParam ? (ui.pins[0] ?? null) : null;
    pinShortcutParam = false;
    return pin;
  },
  keep_screen_on: () => null,

  // `?platform=android&links=1` : liens TIDAL déjà donnés à l'app.
  link_handling: () =>
    params.get("platform") === "android" ? { enabled: params.get("links") === "1" } : null,
  open_link_settings: () => undefined,
  // Le navigateur garde sa taille : seul l'état change (le bouton suit).
  fullscreen: () => fullscreen,
  set_fullscreen: ({ fullscreen: on }) => {
    fullscreen = Boolean(on);
    emit("fullscreen", fullscreen);
    return fullscreen;
  },
  // Hors Tauri, une seule page : le mini-lecteur se prévisualise avec `?mini`.
  open_mini_player: () => undefined,
  close_mini_player: () => undefined,
  mini_player_on_top: () => miniOnTop,
  set_mini_player_on_top: ({ onTop }) => {
    miniOnTop = Boolean(onTop);
    return miniOnTop;
  },
  set_autostart: ({ enabled }) => {
    autostart = Boolean(enabled);
    return autostart;
  },
};
