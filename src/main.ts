import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import "./themes/acidwave.css";
import "./themes/surreal.css";
import "./themes/solarpunk.css";
import "./themes/cyberpunk.css";
import "./themes/neumorph.css";
import "./themes/terminal.css";
import "./themes/riso.css";

const target = document.getElementById("app");
if (!target) throw new Error("#app introuvable");

export default mount(App, { target });
