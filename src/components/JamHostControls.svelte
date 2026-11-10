<!-- Contrôles de l'hôte de la Jam (#288), repliés par défaut pour ne pas encombrer un petit
     écran : lecture / pause par les invités, ajouts suspendus, albums, limite par invité,
     liste des invités avec exclusion. Classes `.setting-row`, `.setting-switch`,
     `.cache-sizes` et `.update-check` reprises de Réglages : les thèmes les habillent déjà. -->
<script lang="ts">
  import type { JamRules, JamStatus } from "../lib/ipc";
  import { setJamAddsOpen, setJamGuestBanned, setJamRules } from "../lib/stores/jam.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  let { status }: { status: JamStatus } = $props();

  /** Limites proposées ; `null` = sans limite. */
  const LIMITS: (number | null)[] = [null, 1, 2, 3, 5, 10];

  let open = $state(false);

  async function run(action: () => Promise<void>) {
    try {
      await action();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  function setRule(change: Partial<JamRules>) {
    void run(() => setJamRules({ ...status.rules, ...change }));
  }
</script>

<div class="jam-host">
  <button
    class="update-check jam-host-toggle"
    aria-expanded={open}
    aria-controls="jam-host-body"
    onclick={() => (open = !open)}
  >
    <span class="chevron" class:open aria-hidden="true">›</span>
    {t.jam.host.toggle}
  </button>

  {#if open}
    <div id="jam-host-body" class="jam-host-body">
      {#snippet toggle(label: string, hint: string, on: boolean, flip: () => void)}
        <div class="setting-row jam-toggle">
          <span class="text">
            <span class="setting-label">{label}</span>
            <span class="hint">{hint}</span>
          </span>
          <button
            class="setting-switch"
            class:on
            role="switch"
            aria-checked={on}
            aria-label={label}
            onclick={flip}
          >
            <span class="knob"></span>
          </button>
        </div>
      {/snippet}

      {@render toggle(t.jam.host.addsOpen, t.jam.host.addsOpenHint, status.addsOpen, () =>
        run(() => setJamAddsOpen(!status.addsOpen)),
      )}
      {@render toggle(t.jam.host.albums, t.jam.host.albumsHint, status.rules.albums, () =>
        setRule({ albums: !status.rules.albums }),
      )}
      {@render toggle(t.jam.guestControls, t.jam.guestControlsHint, status.rules.guestControls, () =>
        setRule({ guestControls: !status.rules.guestControls }),
      )}

      <div class="setting-row jam-limit">
        <span class="text">
          <span class="setting-label" id="jam-limit-label">{t.jam.host.maxPending}</span>
          <span class="hint">{t.jam.host.maxPendingHint}</span>
        </span>
        <div class="cache-sizes jam-limits" role="radiogroup" aria-labelledby="jam-limit-label">
          {#each LIMITS as limit (limit)}
            <button
              class="cache-size jam-limit-choice"
              class:selected={status.rules.maxPending === limit}
              role="radio"
              aria-checked={status.rules.maxPending === limit}
              aria-label={limit === null ? t.jam.host.unlimited : undefined}
              title={limit === null ? t.jam.host.unlimited : undefined}
              onclick={() => setRule({ maxPending: limit })}
            >
              <!-- « ∞ » : les six choix tiennent sur une ligne de téléphone. -->
              {limit ?? "∞"}
            </button>
          {/each}
        </div>
      </div>

      <div class="setting-row jam-guests">
        <span class="setting-label">{t.jam.host.guests}</span>
        {#if status.members.length === 0}
          <p class="hint">{t.jam.host.noGuests}</p>
        {:else}
          <ul class="jam-guest-list">
            {#each status.members as guest (guest.id)}
              <li class="jam-guest" class:banned={guest.banned}>
                <span class="text">
                  <span class="jam-guest-name">{guest.name ?? t.jam.host.anonymous}</span>
                  <span class="hint">
                    {t.jam.host.added(guest.added)}{guest.banned ? ` · ${t.jam.host.banned}` : ""}
                  </span>
                </span>
                <button
                  class="update-check"
                  onclick={() => run(() => setJamGuestBanned(guest.id, !guest.banned))}
                >
                  {guest.banned ? t.jam.host.unban : t.jam.host.ban}
                </button>
              </li>
            {/each}
          </ul>
          <p class="hint">{t.jam.host.banHint}</p>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .jam-host {
    margin-top: 8px;
    max-width: 640px;
  }

  .jam-host-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .chevron {
    display: inline-block;
    transition: transform var(--motion-fast);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .jam-host-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 8px;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 24px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .setting-row.jam-toggle {
    flex-wrap: nowrap;
  }

  .setting-row.jam-guests {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .setting-label {
    font-weight: 500;
  }

  /* Interrupteur repris de Réglages : les thèmes l'habillent déjà par son nom de classe. */
  .setting-switch {
    position: relative;
    flex-shrink: 0;
    width: 40px;
    height: 22px;
    border-radius: 999px;
    background: var(--bg-hover);
    box-shadow: inset 0 0 0 1px var(--border);
    transition: background var(--motion-fast);
  }

  .setting-switch .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text-muted);
    transition:
      transform var(--motion-fast),
      background var(--motion-fast);
  }

  .setting-switch.on {
    background: var(--accent);
  }

  .setting-switch.on .knob {
    transform: translateX(18px);
    background: var(--accent-contrast);
  }

  /* Sélecteur repris de la taille du cache de Réglages, qui passe à la ligne sur un téléphone. */
  .cache-sizes {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
  }

  /* Plus serré que dans Réglages : les six choix tiennent sur une ligne d'un téléphone. */
  .cache-size {
    min-width: 32px;
    padding: 6px 10px;
    border-radius: var(--radius-pill);
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--text-muted);
  }

  .cache-size:hover:not(:disabled) {
    color: var(--text);
    background: var(--bg-hover);
  }

  .cache-size.selected {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .cache-size.selected:hover:not(:disabled) {
    color: var(--accent-contrast);
    background: var(--accent-strong);
  }

  .update-check {
    flex-shrink: 0;
    padding: 6px 14px;
    border: 1px solid var(--border) !important;
    border-radius: 999px;
    font-weight: 600;
    background: var(--control-bg, none);
  }

  .update-check:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .jam-guest-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .jam-guest {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .jam-guest-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .jam-guest.banned .jam-guest-name {
    color: var(--text-muted);
    text-decoration: line-through;
  }

  /* Studio : angles vifs. Atelier : angles à peine arrondis, pas de fond. */
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .update-check {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .setting-row {
    border-radius: 2px;
    background: transparent;
  }
</style>
