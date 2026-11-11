<script lang="ts">
  // Journal des versions (#340), dans Réglages › Application. Le backend renvoie les
  // versions déjà filtrées pour la plateforme.
  import { commands, type Release } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  let releases = $state<Release[] | null>(null);

  // Monté à l'ouverture (bouton de Réglages › Application).
  $effect(() => {
    commands.changelog().then(
      (r) => (releases = r),
      (e) => pushToast({ level: "error", message: String(e) }),
    );
  });

  /** Découpe une entrée autour des `code` en ligne du Markdown. */
  function parts(entry: string): { text: string; code: boolean }[] {
    return entry.split("`").map((text, i) => ({ text, code: i % 2 === 1 }));
  }
</script>

{#if releases}
  <div class="changelog">
    {#each releases as release (release.version)}
      <section>
        <h3>
          <span class="mono">v{release.version}</span>
          {#if release.published}<span class="published">{t.settings.publishedOn(release.published)}</span>{/if}
        </h3>
        <ul>
          {#each release.entries as entry, i (i)}
            <li>
              {#each parts(entry) as part, j (j)}{#if part.code}<code>{part.text}</code>{:else}{part.text}{/if}{/each}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
{/if}

<style>
  .changelog {
    max-height: 420px;
    overflow-y: auto;
    margin: 12px 0;
    padding-right: 8px;
  }
  section + section {
    margin-top: 14px;
  }
  h3 {
    display: flex;
    gap: 10px;
    align-items: baseline;
    margin: 0 0 6px;
    font-size: 0.95em;
  }
  .published {
    color: var(--text-muted);
    font-size: 0.85em;
    font-weight: normal;
  }
  ul {
    margin: 0;
    padding-left: 18px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  code {
    font-family: var(--font-mono, monospace);
    font-size: 0.9em;
  }
</style>
