<script lang="ts">
  // Fond propre à chaque thème, derrière le shell ou derrière la vue « En lecture ».
  import { ui } from "../../lib/stores/ui.svelte";
  import Ambient from "../Ambient.svelte";
  import AcidField from "./AcidField.svelte";
  import SurrealScene from "./SurrealScene.svelte";
  import SolarScene from "./SolarScene.svelte";
  import CyberScene from "./CyberScene.svelte";
  import RisoPaper from "./RisoPaper.svelte";
  import BauhausComposition from "./BauhausComposition.svelte";
  import AeroLivery from "./AeroLivery.svelte";
  import AlloyBody from "./AlloyBody.svelte";
  import EstampeScene from "./EstampeScene.svelte";
  import DecoScene from "./DecoScene.svelte";
  import CassetteDeck from "./CassetteDeck.svelte";
  import SeanceHall from "./SeanceHall.svelte";
  import OnboardDeck from "./OnboardDeck.svelte";
  import NoirBlinds from "./NoirBlinds.svelte";

  let { context = "shell" }: { context?: "shell" | "nowplaying" } = $props();

  const theme = $derived(ui.settings.theme);
  const intense = $derived(context === "nowplaying");
</script>

<div class="backdrop">
  {#if theme === "seaglass" || (theme === "studio" && intense)}
    <Ambient intensity={intense ? 1.4 : 1} />
  {:else if theme === "acidwave"}
    <AcidField {intense} />
  {:else if theme === "surreal"}
    <SurrealScene {intense} />
  {:else if theme === "solarpunk"}
    <SolarScene {intense} />
  {:else if theme === "cyberpunk"}
    <CyberScene {intense} />
  {:else if theme === "riso"}
    <RisoPaper {intense} />
  {:else if theme === "bauhaus"}
    <BauhausComposition {intense} />
  {:else if theme === "aero"}
    <AeroLivery {intense} />
  {:else if theme === "alloy" || theme === "alloy-dark"}
    <AlloyBody {intense} />
  {:else if theme === "estampe"}
    <EstampeScene {intense} />
  {:else if theme === "artdeco"}
    <DecoScene {intense} />
  {:else if theme === "cassette"}
    <CassetteDeck {intense} />
  {:else if theme === "seance"}
    <SeanceHall {intense} />
  {:else if theme === "onboard"}
    <OnboardDeck {intense} />
  {:else if theme === "noir"}
    <NoirBlinds {intense} />
  {/if}
</div>

<style>
  /* Sans boîte : les fonds se placent comme avant par rapport au shell. */
  .backdrop {
    display: contents;
  }

  /* Android (#158) : fonds figés. Leurs animations (nappes floutées, grilles, pluie…)
     faisaient recomposer l'écran à chaque image pendant la lecture (140 % de CPU sur le
     S21 en Acidwave) ; seule la progression bouge. Vaut aussi pour les fonds à venir. */
  :global(:root[data-platform="android"]) .backdrop :global(*),
  :global(:root[data-platform="android"]) .backdrop :global(*::before),
  :global(:root[data-platform="android"]) .backdrop :global(*::after) {
    animation: none !important;
  }
</style>
