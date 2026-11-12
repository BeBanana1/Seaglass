<!-- Miniature d'un thème (#390), dessinée en CSS avec les couleurs d'origine du thème, quel que
     soit le thème courant. `large` dans le bloc du thème actuel, `small` dans la galerie. -->
<script lang="ts">
  import type { Theme } from "../lib/ipc";

  let { theme, size = "small" }: { theme: Theme; size?: "small" | "large" } = $props();
</script>

<span class="preview {theme} {size}" aria-hidden="true">
  <span class="side"></span>
  <span class="main"><span class="h"></span><span class="l"></span><span class="l short"></span></span>
  <span class="bar"></span>
</span>

<style>
  .preview {
    display: grid;
    grid-template-columns: 22% 1fr;
    grid-template-rows: 1fr 18%;
    gap: 4px;
    width: 100%;
    padding: 6px;
    border-radius: 6px;
    overflow: hidden;
  }

  /* Tailles avant les règles des thèmes, qui peuvent changer marges et écarts. */
  .preview.small {
    height: 72px;
  }

  .preview.large {
    height: 132px;
  }

  .small .main {
    gap: 4px;
    padding: 6px;
  }

  .small .h {
    height: 7px;
  }

  .small .l {
    height: 4px;
  }

  .side {
    grid-row: 1;
    border-radius: 4px;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 8px;
    border-radius: 4px;
  }

  .h {
    width: 60%;
    height: 9px;
    border-radius: 2px;
  }

  .l {
    width: 85%;
    height: 5px;
    border-radius: 2px;
    opacity: 0.6;
  }

  .l.short {
    width: 50%;
  }

  .bar {
    grid-column: 1 / -1;
    border-radius: 4px;
  }

  .preview.seaglass {
    background:
      radial-gradient(circle at 20% 20%, #4f95b0, transparent 60%),
      radial-gradient(circle at 80% 90%, #2c5c6b, transparent 60%),
      #061012;
  }

  .seaglass .side,
  .seaglass .main,
  .seaglass .bar {
    background: rgb(255 255 255 / 0.12);
    border: 1px solid rgb(255 255 255 / 0.12);
  }

  .seaglass .h {
    background: #e9fffa;
  }

  .seaglass .l {
    background: #b8d8d0;
  }

  .preview.studio {
    gap: 1px;
    padding: 0;
    border-radius: 0;
    background: #26262b;
  }

  .studio .side,
  .studio .main,
  .studio .bar {
    border-radius: 0;
    background: #0f0f11;
  }

  .studio .h {
    background: #ffb547;
  }

  .studio .l {
    background: #85858f;
  }

  .preview.atelier {
    gap: 0;
    padding: 0;
    background: #f3efe6;
    border: 1px solid #dcd3c1;
  }

  .atelier .side {
    border-right: 1px solid #dcd3c1;
    border-radius: 0;
  }

  .atelier .bar {
    border-top: 1px solid #1c1a16;
    border-radius: 0;
  }

  .atelier .h {
    height: 12px;
    background: #1c1a16;
  }

  .atelier .l {
    background: #6d675b;
  }

  .preview.acidwave {
    gap: 3px;
    background:
      radial-gradient(circle at 15% 15%, rgb(200 255 0 / 0.55), transparent 50%),
      radial-gradient(circle at 85% 70%, rgb(255 43 214 / 0.55), transparent 50%),
      #050507;
  }

  .acidwave .side,
  .acidwave .main,
  .acidwave .bar {
    border-radius: 2px;
    background: rgb(8 8 12 / 0.75);
    border: 1px solid rgb(200 255 0 / 0.35);
  }

  .acidwave .h {
    background: linear-gradient(180deg, #fff, #4d5560 50%, #eef3f7);
    box-shadow: 0 0 6px rgb(200 255 0 / 0.6);
  }

  .acidwave .l {
    background: #c8ff00;
  }

  .preview.surreal {
    position: relative;
    gap: 5px;
    padding: 8px;
    border-radius: 12px;
    background:
      radial-gradient(circle at 72% 62%, #f6b38a 0 16%, transparent 17%),
      linear-gradient(180deg, #e8d9f0, #fbe7d8 70%);
  }

  .surreal .side,
  .surreal .main,
  .surreal .bar {
    border-radius: 10px;
    background: rgb(255 250 244 / 0.8);
    box-shadow: 0 6px 10px -6px rgb(61 42 77 / 0.5);
  }

  .surreal .h {
    background: #3d2a4d;
    border-radius: 6px;
  }

  .surreal .l {
    background: #b49cbf;
  }

  .preview.solarpunk {
    gap: 5px;
    padding: 8px;
    border-radius: 12px;
    background:
      radial-gradient(circle at 82% 18%, #f3cf6b 0 14%, transparent 15%),
      radial-gradient(ellipse at 0% 100%, #2e6b3d 0 26%, transparent 27%),
      radial-gradient(ellipse at 100% 100%, #3f8a4f 0 22%, transparent 23%),
      linear-gradient(180deg, #f8efcc, #e6eecf 75%);
  }

  .solarpunk .side,
  .solarpunk .main,
  .solarpunk .bar {
    border-radius: 9px;
    background: rgb(251 249 236 / 0.82);
    box-shadow: 0 6px 10px -6px rgb(24 64 36 / 0.45);
  }

  .solarpunk .main {
    border-radius: 18px 18px 9px 9px;
  }

  .solarpunk .h {
    background: #15301d;
    border-radius: 6px;
  }

  .solarpunk .l {
    background: #2c7a41;
  }

  .preview.cyberpunk {
    gap: 3px;
    border-radius: 0;
    background:
      linear-gradient(180deg, transparent 62%, rgb(255 42 109 / 0.35)),
      repeating-linear-gradient(0deg, rgb(0 0 0 / 0.35) 0 1px, transparent 1px 3px),
      #06070d;
  }

  .cyberpunk .side,
  .cyberpunk .main,
  .cyberpunk .bar {
    border-radius: 0;
    background: rgb(9 11 20 / 0.85);
    border: 1px solid rgb(0 229 255 / 0.45);
  }

  .cyberpunk .main {
    clip-path: polygon(0 0, calc(100% - 8px) 0, 100% 8px, 100% 100%, 8px 100%, 0 calc(100% - 8px));
  }

  .cyberpunk .h {
    background: #fcee0a;
    box-shadow: 2px 0 0 rgb(255 42 109 / 0.7);
  }

  .cyberpunk .l {
    background: #00e5ff;
  }

  .preview.neumorph,
  .preview.neumorph-dark {
    gap: 7px;
    padding: 9px;
    border-radius: 12px;
    background: #e3e7ed;
  }

  .preview.neumorph-dark {
    background: #24272e;
  }

  .neumorph :is(.side, .main, .bar) {
    border-radius: 8px;
    background: #e3e7ed;
    box-shadow:
      3px 3px 6px rgb(150 162 184 / 0.6),
      -3px -3px 6px rgb(255 255 255 / 0.9);
  }

  .neumorph-dark :is(.side, .main, .bar) {
    border-radius: 8px;
    background: #24272e;
    box-shadow:
      3px 3px 6px rgb(0 0 0 / 0.5),
      -3px -3px 6px rgb(255 255 255 / 0.06);
  }

  .neumorph .h {
    background: #252b36;
  }

  .neumorph .l {
    background: #3f56c9;
  }

  .neumorph-dark .h {
    background: #e4e8ef;
  }

  .neumorph-dark .l {
    background: #8fa2ff;
  }

  .preview.terminal {
    gap: 3px;
    border-radius: 0;
    background:
      repeating-linear-gradient(0deg, rgb(0 0 0 / 0.3) 0 1px, transparent 1px 3px),
      radial-gradient(ellipse at center, #0a130a, #020402 80%);
  }

  .terminal :is(.side, .main, .bar) {
    border-radius: 0;
    background: transparent;
    border: 1px solid rgb(65 255 122 / 0.45);
  }

  .terminal .h {
    width: 55%;
    background: #41ff7a;
    box-shadow: 0 0 6px rgb(65 255 122 / 0.7);
  }

  .preview.riso {
    gap: 5px;
    padding: 8px;
    border-radius: 4px;
    background:
      radial-gradient(circle at 12% 18%, rgb(200 22 111 / 0.55) 0 22%, transparent 23%),
      radial-gradient(circle at 88% 88%, rgb(11 95 165 / 0.5) 0 20%, transparent 21%),
      #f4efe4;
  }

  .riso :is(.side, .main, .bar) {
    border-radius: 3px;
    background: #faf7f0;
    border: 1.5px solid #1d1a2b;
    box-shadow: 3px 3px 0 rgb(11 95 165 / 0.45);
  }

  .riso .h {
    background: #1d1a2b;
    box-shadow: 2px 1px 0 rgb(200 22 111 / 0.6);
  }

  .riso .l {
    background: linear-gradient(transparent 40%, rgb(200 22 111 / 0.5) 40%);
  }

  .preview.bauhaus {
    gap: 4px;
    padding: 7px;
    border-radius: 0;
    background:
      radial-gradient(circle at 92% 8%, #c8231a 0 24%, transparent 25%),
      radial-gradient(circle at 0% 100%, #f2b80d 0 26%, transparent 27%),
      #f1ece0;
  }

  .bauhaus :is(.side, .main, .bar) {
    border-radius: 0;
    background: #f7f3ea;
    border: 1.5px solid #141414;
  }

  .bauhaus .h {
    background: #141414;
  }

  .bauhaus .l {
    background: #1d4fa0;
  }

  .preview.aero {
    gap: 4px;
    border-radius: 0;
    background:
      linear-gradient(90deg, transparent 88%, #f2ee1d 88%),
      radial-gradient(circle, rgb(31 47 53 / 0.25) 0.8px, transparent 1.2px) 0 0 / 6px 6px,
      #d3eaee;
  }

  .aero :is(.side, .main, .bar) {
    border-radius: 0;
    background: #e3f2f4;
  }

  .aero .h {
    background: #1f2f35;
    box-shadow: 0 4px 0 -1px #e3f2f4, 0 6px 0 -1px #1f2f35;
  }

  .aero .l {
    background: #465b62;
  }

  .aero .bar {
    box-shadow: inset 4px 0 0 #f2ee1d, inset 6px 0 0 #1f2f35;
  }

  .terminal .l {
    background: repeating-linear-gradient(90deg, rgb(65 255 122 / 0.6) 0 4px, transparent 4px 6px);
  }

  .preview.alloy,
  .preview.alloy-dark {
    gap: 4px;
    border-radius: 0;
    background: linear-gradient(172deg, #eef1f3, #c6ccd2 45%, #dde1e5 55%, #b3bac2);
  }

  .preview.alloy-dark {
    background: linear-gradient(172deg, #3c4148, #22262b 45%, #30353b 55%, #191c20);
  }

  :is(.alloy, .alloy-dark) :is(.side, .main) {
    border-radius: 0;
    background: rgb(234 237 240 / 0.6);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.7);
    clip-path: polygon(0 0, calc(100% - 6px) 0, 100% 6px, 100% 100%, 0 100%);
  }

  .alloy-dark :is(.side, .main) {
    background: rgb(38 42 48 / 0.85);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.07);
  }

  :is(.alloy, .alloy-dark) .bar {
    border-radius: 0;
    background:
      linear-gradient(90deg, #c3261f 0 10px, transparent 10px),
      repeating-linear-gradient(-55deg, rgb(255 255 255 / 0.18) 0 1px, transparent 1px 4px) right 4px center / 18px 70% no-repeat,
      #121417;
    clip-path: polygon(6px 0, 100% 0, 100% 100%, 0 100%, 0 6px);
  }

  .alloy .h {
    background: #15181c;
  }

  .alloy .l {
    background: #4a5159;
  }

  .alloy-dark .h {
    background: #e6e9ec;
  }

  .alloy-dark .l {
    background: #ffb21e;
  }

  .preview.midnight {
    background: #000;
  }

  .midnight :is(.side, .main, .bar) {
    background: #000;
    border: 1px solid rgb(255 255 255 / 0.16);
  }

  .midnight .h {
    background: #d4d4d4;
  }

  .midnight .l {
    background: #8c8c8c;
  }

  .preview.estampe {
    border-radius: 0;
    background:
      radial-gradient(circle at 86% 22%, #c13b24 0 9%, transparent 10%),
      repeating-radial-gradient(circle at 0 100%, #2f5f84 0 2px, transparent 2px 6px) left bottom / 40% 45% no-repeat,
      linear-gradient(#e4dcc9, #f2eadb 30%);
  }

  .estampe :is(.side, .main, .bar) {
    border-radius: 0;
    background: rgb(248 243 231 / 0.9);
    border: 1px solid rgb(31 53 80 / 0.55);
    box-shadow: inset 0 0 0 2px #f8f3e7, inset 0 0 0 3px rgb(31 53 80 / 0.2);
  }

  .estampe .h {
    background: #1b2a3d;
  }

  .estampe .l {
    background: #2f5f84;
  }

  .preview.artdeco {
    border-radius: 0;
    background:
      repeating-conic-gradient(from -60deg at 50% 120%, rgb(201 162 74 / 0.18) 0 2deg, transparent 2deg 8deg) 0 0 / 100% 100%,
      #0a111e;
  }

  .artdeco :is(.side, .main, .bar) {
    border-radius: 0;
    background: rgb(12 20 35 / 0.92);
    border: 1px solid rgb(201 162 74 / 0.5);
    box-shadow: inset 0 0 0 2px #0c1423, inset 0 0 0 3px rgb(201 162 74 / 0.2);
  }

  .artdeco .h {
    background: #ece4d0;
  }

  .artdeco .l {
    background: #c9a24a;
  }

  .preview.cassette {
    border-radius: 3px;
    background:
      linear-gradient(transparent 70%, #d8692a 70% 76%, #d9a336 76% 82%, #7d7f35 82% 88%, transparent 88%),
      #24180f;
  }

  .cassette :is(.side, .main, .bar) {
    border-radius: 3px;
    background: linear-gradient(#d8692a 0 2px, #d9a336 2px 4px, #7d7f35 4px 6px, #34251a 6px);
    border: 1px solid rgb(0 0 0 / 0.6);
  }

  .cassette .h {
    background: #f1e3c4;
  }

  .cassette .l {
    background: #ef8a3c;
  }

  /* Rideaux de velours sur les bords, salle noire. */
  .preview.seance {
    border-radius: 6px;
    background:
      linear-gradient(90deg, #7c141c 0 7%, transparent 7% 93%, #7c141c 93%),
      #0e0a0b;
  }

  .seance :is(.side, .main, .bar) {
    border-radius: 4px;
    background: #1a1214;
    border: 1px solid rgb(239 230 210 / 0.1);
  }

  .seance .h {
    background: #efe6d2;
  }

  .seance .l {
    background: #e65a5e;
  }

  /* La pochette, seule touche de couleur. */
  .midnight .bar {
    background: linear-gradient(90deg, #4f95b0 0 14%, #000 14%);
  }
</style>
