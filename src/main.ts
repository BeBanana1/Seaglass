import { mount } from "svelte";
import App from "./App.svelte";
import { initPlatform } from "./lib/platform.svelte";
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

const target = document.getElementById("app");
if (!target) throw new Error("#app introuvable");

// La plateforme choisit la mise en page (tactile sur Android) : lue avant le premier rendu.
initPlatform().finally(() => mount(App, { target }));
