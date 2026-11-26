// Settings: page, language, appearance and accent color. The theme catalog and the other
// tabs' strings are in `settings/`, spread here so that `t.settings.*` stays flat.

import * as app from "./settings/app";
import * as playback from "./settings/playback";
import * as themes from "./settings/themes";

export const fr = {
  title: "Réglages",
  sections: { appearance: "Apparence", playback: "Lecture", app: "Application" },

  language: "Langue",
  languageHint: "Langue de l'interface et des contenus proposés par le service (accueil, mixes).",
  languages: { system: "Système", fr: "Français", en: "English" },

  zoom: "Taille de l'interface",
  zoomHint:
    "Agrandit ou réduit tout l'affichage. Raccourcis : Ctrl + « + », Ctrl + « - », Ctrl + 0.",
  motion: "Animations",
  motionHint:
    "Fonds animés, fondus et transitions. « Système » suit le réglage « réduire les " +
    "animations » de l'appareil.",
  motions: { system: "Système", reduced: "Réduites", full: "Complètes" },
  eggs: "Clins d'œil",
  eggsHint: "Petites surprises discrètes selon la date ou certaines recherches. Jamais de son.",
  theme: "Thème",
  themeHint: "Le thème s'applique tout de suite et reste enregistré.",
  themeVariant: "Variante",
  themeSchemes: { light: "Clair", dark: "Sombre" },
  themeFavoriteAdd: "Ajouter aux favoris",
  themeFavoriteRemove: "Retirer des favoris",
  themeGallery: "Tous les thèmes",
  themeFilter: "Afficher",
  themeFilters: { all: "Tous", favorites: "Favoris", light: "Clairs", dark: "Sombres" },
  themeNoFavorites: "Aucun favori pour l'instant : l'étoile du thème actuel l'ajoute ici.",

  themes: themes.fr,

  accent: "Couleur d'accent",
  accentHint:
    "Boutons, éléments actifs et titres mis en avant. Par défaut, chaque thème a la sienne " +
    "(accordée à l'album pour Verre de mer, Atelier et Surréalisme). Le choix vaut pour le thème actuel : " +
    "chaque thème garde sa couleur.",
  accentTheme: "Celle du thème",
  accentOther: "Autre couleur",
  accentOtherShort: "Autre…",

  ...playback.fr,
  ...app.fr,
};

export const en: typeof fr = {
  title: "Settings",
  sections: { appearance: "Appearance", playback: "Playback", app: "Application" },

  language: "Language",
  languageHint:
    "Language of the interface and of the content suggested by the service (home, mixes).",
  languages: { system: "System", fr: "Français", en: "English" },

  zoom: "Interface size",
  zoomHint: 'Enlarges or shrinks the whole display. Shortcuts: Ctrl + "+", Ctrl + "-", Ctrl + 0.',
  motion: "Animations",
  motionHint:
    'Animated backdrops, fades and transitions. "System" follows the device\'s ' +
    '"reduce motion" setting.',
  motions: { system: "System", reduced: "Reduced", full: "Full" },
  eggs: "Easter eggs",
  eggsHint: "Small, discreet surprises on certain dates or searches. Never any sound.",
  theme: "Theme",
  themeHint: "The theme applies right away and is saved.",
  themeVariant: "Variant",
  themeSchemes: { light: "Light", dark: "Dark" },
  themeFavoriteAdd: "Add to favorites",
  themeFavoriteRemove: "Remove from favorites",
  themeGallery: "All themes",
  themeFilter: "Show",
  themeFilters: { all: "All", favorites: "Favorites", light: "Light", dark: "Dark" },
  themeNoFavorites: "No favorites yet: the star on the current theme adds it here.",

  themes: themes.en,

  accent: "Accent color",
  accentHint:
    "Buttons, active items and highlighted titles. By default, each theme has its own " +
    "(matched to the album for Seaglass, Atelier and Surrealism). The choice applies to the current theme: " +
    "each theme keeps its own color.",
  accentTheme: "Theme default",
  accentOther: "Other color",
  accentOtherShort: "Other…",

  ...playback.en,
  ...app.en,
};
