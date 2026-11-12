<script lang="ts">
  // Clin d'œil (#399) : 7 clics sur la version (Réglages › Application) font défiler les
  // remerciements et les licences comme un générique de fin. Un clic ou Échap le ferme, il
  // se ferme seul à la fin. Animations réduites : la liste fixe.
  import { t } from "../lib/i18n.svelte";

  let { onClose }: { onClose: () => void } = $props();

  /** Monté sous `<body>` : un panneau parent (flou, `contain`) bornerait `position: fixed`. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="credits" role="dialog" aria-modal="true" aria-label={t.eggs.creditsTitle} tabindex="-1" onclick={onClose} use:portal>
  <div class="roll" onanimationend={onClose}>
    <p class="title display">Seaglass</p>
    <p class="version mono">v{__APP_VERSION__}</p>
    {#each t.eggs.credits as [role, names] (role)}
      <p class="role">{role}</p>
      <p class="names">{names}</p>
    {/each}
    <p class="end display">{t.eggs.creditsEnd}</p>
  </div>
</div>

<style>
  .credits {
    position: fixed;
    inset: 0;
    z-index: 1000;
    overflow: hidden;
    background: #000;
    color: #f2efe6;
    cursor: pointer;
    text-align: center;
  }

  .roll {
    position: absolute;
    left: 0;
    right: 0;
    top: 70%;
    animation: roll 22s linear 1 forwards;
  }

  @keyframes roll {
    to {
      transform: translateY(calc(-100% - 100vh));
    }
  }

  p {
    margin: 0;
  }

  .title {
    font-size: 40px;
  }

  .version {
    margin-bottom: 56px;
    opacity: 0.6;
  }

  .role {
    margin-top: 28px;
    font-size: 11px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    opacity: 0.6;
  }

  .names {
    font-size: 18px;
  }

  .end {
    margin-top: 80px;
    font-size: 22px;
  }

  :global(:root[data-motion="reduced"]) .credits {
    overflow: auto;
  }

  :global(:root[data-motion="reduced"]) .roll {
    position: static;
    padding: 48px 0;
    animation: none;
  }
</style>
