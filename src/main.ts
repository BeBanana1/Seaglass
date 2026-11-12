import { mount } from "svelte";
import App from "./App.svelte";
import MiniWindow from "./views/MiniWindow.svelte";
import { initPlatform, isMiniWindow } from "./lib/platform.svelte";
import "./app.css";
import "./themes/acidwave.css";
import "./themes/surreal.css";
import "./themes/solarpunk.css";
import "./themes/cyberpunk.css";
import "./themes/neumorph.css";
import "./themes/terminal.css";
import "./themes/riso.css";
import "./themes/bauhaus.css";
import "./themes/aero.css";
import "./themes/alloy.css";

const target = document.getElementById("app");
if (!target) throw new Error("#app introuvable");

// La plateforme choisit la mise en page (tactile sur Android) : lue avant le premier rendu.
// La fenêtre du mini-lecteur (#384) monte sa propre vue, sans le reste de l'app.
const Root = isMiniWindow() ? MiniWindow : App;
initPlatform().finally(() => mount(Root, { target }));
