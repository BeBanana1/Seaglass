<!-- Bandeau non bloquant du réglage « Demander » (#59) : le backend propose une édition
     hi-res du titre courant dans `player-state` ; la réponse vaut pour tout l'album. -->
<script lang="ts">
  import { commands } from "../lib/ipc";
  import { khz } from "../lib/format";
  import { player } from "../lib/stores/player.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import Icon from "./Icon.svelte";

  const offer = $derived(player.state.hiresOffer);
  const format = $derived(
    offer?.bitDepth && offer.sampleRate ? `${offer.bitDepth}/${khz(offer.sampleRate)}` : "hi-res",
  );

  async function answer(accept: boolean) {
    try {
      await commands.answerHiresOffer(accept);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

{#if offer}
  <div class="hires-offer" role="status">
    <span class="text">
      <span class="hires-offer-title">Édition {format} disponible</span>
      {#if offer.hiresEdition}<span class="hires-offer-album">{offer.hiresEdition}</span>{/if}
    </span>
    <button class="hires-offer-accept" onclick={() => answer(true)}>Écouter en {format}</button>
    <button
      class="hires-offer-dismiss"
      onclick={() => answer(false)}
      title="Ne plus proposer pour cet album"
      aria-label="Ne plus proposer pour cet album"
    >
      <Icon name="close" size={14} />
    </button>
  </div>
{/if}

<style>
  .hires-offer {
    position: absolute;
    bottom: calc(100% + 10px);
    left: 18px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: min(560px, calc(100% - 36px));
    padding: 8px 8px 8px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--dialog-bg);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.35);
    font-size: 13px;
    animation: rise 0.2s ease-out;
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .hires-offer-title {
    font-weight: 600;
  }

  .hires-offer-album {
    overflow: hidden;
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .hires-offer-accept {
    flex-shrink: 0;
    padding: 6px 12px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .hires-offer-accept:hover {
    background: var(--accent-strong);
  }

  .hires-offer-dismiss {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    padding: 6px;
    border-radius: 50%;
    color: var(--text-muted);
  }

  .hires-offer-dismiss:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  :global([data-theme="studio"]) .hires-offer,
  :global([data-theme="atelier"]) .hires-offer {
    border-radius: 0;
  }

  :global([data-theme="studio"]) .hires-offer-accept,
  :global([data-theme="atelier"]) .hires-offer-accept {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .hires-offer {
    border-color: var(--text);
    box-shadow: 0 10px 28px rgb(60 45 20 / 0.18);
  }
</style>
