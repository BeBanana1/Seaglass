<script lang="ts">
  // Fond propre à chaque thème, derrière le shell ou derrière la vue « En lecture ».
  import { ui } from "../../lib/stores/ui.svelte";
  import Ambient from "../Ambient.svelte";
  import AcidField from "./AcidField.svelte";
  import SurrealScene from "./SurrealScene.svelte";
  import SolarScene from "./SolarScene.svelte";

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
{/if}
