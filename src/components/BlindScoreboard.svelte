<script lang="ts">
  // Classement d'un blind test à plusieurs (#441) : rang, joueur, points de la manche
  // révélée (ou « a répondu » pendant la manche), score. Tout vient du backend.
  import type { BlindPlayer, BlindPlayerResult } from "../lib/ipc";
  import { t } from "../lib/i18n.svelte";

  let {
    players,
    round = null,
    showAnswered = false,
  }: {
    players: BlindPlayer[];
    /** Résultats de la manche révélée, affichés à côté du score. */
    round?: BlindPlayerResult[] | null;
    showAnswered?: boolean;
  } = $props();

  function name(p: { id: number; name: string | null; host: boolean }): string {
    return p.host ? t.blindTest.host : (p.name ?? t.blindTest.guest(p.id));
  }
</script>

<section class="blind-scoreboard">
  <h2>{t.blindTest.ranking}</h2>
  {#if players.length === 0}
    <p class="empty">{t.blindTest.noPlayers}</p>
  {:else}
    <ol>
      {#each players as p, i (p.id)}
        {@const r = round?.find((x) => x.id === p.id)}
        <li class:host={p.host}>
          <span class="rank">{i + 1}</span>
          <span class="who">{name(p)}</span>
          {#if r}
            <span class="gain" class:good={r.points > 0}>
              {r.answered
                ? t.blindTest.points(r.points)
                : t.blindTest.noAnswer}{#if r.fastest}&nbsp;· {t.blindTest.fastest}{/if}
            </span>
          {:else if showAnswered && p.answered}
            <span class="gain good">✓ {t.blindTest.answered}</span>
          {:else}
            <span></span>
          {/if}
          <span class="score">{p.score}</span>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  section {
    width: 100%;
  }

  h2 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
  }

  ol {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: 2em minmax(0, 1fr) auto 3em;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-radius: var(--radius-card, 8px);
  }

  li.host {
    background: var(--bg-hover);
  }

  .rank {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .who {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .gain {
    font-size: 13px;
    color: var(--text-muted);
  }

  .gain.good {
    color: var(--accent);
  }

  .score {
    text-align: right;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
</style>
