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

  let { context = "shell" }: { context?: "shell" | "nowplaying" } = $props();

  const theme = $derived(ui.settings.theme);
  const intense = $derived(context === "nowplaying");
</script>

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
{/if}
