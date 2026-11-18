<script lang="ts">
  // Réglages › Lecture › Égaliseur (#428) : interrupteur, profil choisi, import AutoEQ,
  // édition des filtres et courbe de réponse. Les règles (bornes, analyse du fichier
  // AutoEQ) sont côté Rust (`player::eq`, `player::autoeq`) ; ici, l'affichage et la saisie.
  import { commands, type EqFilter, type EqFilterKind, type EqProfile, type EqSettings } from "../lib/ipc";
  import { effectivePreamp, response } from "../lib/eqCurve";
  import { gainLabel } from "../lib/format";
  import { num, t } from "../lib/i18n.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";

  let {
    settings,
    disabled = false,
    onchange,
  }: {
    settings: EqSettings;
    disabled?: boolean;
    /** Enregistre les nouveaux réglages (le backend les valide et les applique à chaud). */
    onchange: (next: EqSettings) => void;
  } = $props();

  /** Mêmes bornes que `player::eq`, pour guider la saisie ; le backend les revérifie. */
  const MAX_PROFILES = 32;
  const MAX_FILTERS = 20;
  const MAX_FILE = 16 * 1024;
  const KINDS: EqFilterKind[] = ["peaking", "lowShelf", "highShelf"];
  const RANGES = { freq: [20, 20_000], gain: [-20, 20], q: [0.1, 10] } as const;

  const active = $derived(settings.profiles.find((p) => p.id === settings.active) ?? null);
  let importing = $state(false);
  let importText = $state("");
  let importName = $state("");
  let busy = $state(false);

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  function save(next: Partial<EqSettings>) {
    onchange({ ...settings, ...next });
  }

  function nextId(): number {
    return Math.max(0, ...settings.profiles.map((p) => p.id)) + 1;
  }

  function addProfile(profile: EqProfile) {
    if (settings.profiles.length >= MAX_PROFILES) return fail(t.equalizer.maxProfiles(MAX_PROFILES));
    save({ enabled: true, active: profile.id, profiles: [...settings.profiles, profile] });
  }

  function newProfile() {
    const id = nextId();
    addProfile({ id, name: t.equalizer.newProfileName(id), preamp: null, filters: [] });
  }

  function deleteProfile() {
    if (!active) return;
    const profiles = settings.profiles.filter((p) => p.id !== active.id);
    save({ profiles, active: profiles[0]?.id ?? null, enabled: settings.enabled && profiles.length > 0 });
  }

  /** Modifie le profil choisi. */
  function updateActive(change: Partial<EqProfile>) {
    if (!active) return;
    save({ profiles: settings.profiles.map((p) => (p.id === active.id ? { ...p, ...change } : p)) });
  }

  function updateFilter(index: number, change: Partial<EqFilter>) {
    if (!active) return;
    updateActive({ filters: active.filters.map((f, i) => (i === index ? { ...f, ...change } : f)) });
  }

  /** Valeur saisie ramenée dans ses bornes ; `null` si le champ n'est pas un nombre. */
  function clampInput(e: Event, [min, max]: readonly [number, number]): number | null {
    const value = (e.currentTarget as HTMLInputElement).valueAsNumber;
    return Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : null;
  }

  function setNumber(index: number, key: keyof typeof RANGES, e: Event) {
    const value = clampInput(e, RANGES[key]);
    if (value !== null) updateFilter(index, { [key]: value });
  }

  function setPreamp(e: Event) {
    const value = clampInput(e, RANGES.gain);
    if (value !== null) updateActive({ preamp: value });
  }

  function rename(e: Event) {
    const name = (e.currentTarget as HTMLInputElement).value.trim();
    if (name && name !== active?.name) updateActive({ name });
  }

  function addFilter() {
    if (!active || active.filters.length >= MAX_FILTERS) return;
    updateActive({ filters: [...active.filters, { kind: "peaking", freq: 1000, gain: 0, q: 1 }] });
  }

  async function pickFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    if (file.size > MAX_FILE) return fail(t.equalizer.fileTooLarge);
    importText = await file.text();
    if (!importName) importName = file.name.replace(/\.txt$/i, "").replace(/\s*ParametricEQ$/i, "").trim();
  }

  async function confirmImport() {
    busy = true;
    try {
      const parsed = await commands.parseAutoeq(importText);
      const id = nextId();
      addProfile({ ...parsed, id, name: importName.trim() || t.equalizer.newProfileName(id) });
      importing = false;
      importText = "";
      importName = "";
    } catch (e) {
      fail(e);
    } finally {
      busy = false;
    }
  }

  // Courbe : 600 × 160, de +15 à −15 dB.
  const W = 600;
  const H = 160;
  const DB = 15;
  const x = (hz: number) => (Math.log10(hz / 20) / 3) * W;
  const y = (db: number) => H / 2 - (Math.max(-DB, Math.min(DB, db)) / DB) * (H / 2);
  const curve = $derived(
    active
      ? response(active)
          .map(([hz, db], i) => `${i ? "L" : "M"}${x(hz).toFixed(1)},${y(db).toFixed(1)}`)
          .join(" ")
      : "",
  );
  const GRID_HZ = [100, 1000, 10_000];
</script>

<div class="eq-settings">
  <label class="setting-row">
    <span class="text">
      <span class="setting-label">{t.equalizer.enable}</span>
      <span class="hint">{t.equalizer.enableHint}</span>
    </span>
    <button
      class="setting-switch"
      class:on={settings.enabled}
      role="switch"
      aria-checked={settings.enabled}
      aria-label={t.equalizer.enable}
      disabled={disabled || !active}
      onclick={() => save({ enabled: !settings.enabled })}
    >
      <span class="knob"></span>
    </button>
  </label>

  <div class="eq-toolbar">
    {#if settings.profiles.length}
      <label class="field">
        <span class="hint">{t.equalizer.profile}</span>
        <select
          class="eq-select"
          value={settings.active ?? ""}
          {disabled}
          onchange={(e) => save({ active: Number((e.currentTarget as HTMLSelectElement).value) })}
        >
          {#each settings.profiles as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
      </label>
    {:else}
      <p class="hint">{t.equalizer.noProfile}</p>
    {/if}
    <div class="actions">
      <button class="eq-button" {disabled} onclick={() => (importing = !importing)}>{t.equalizer.importAction}</button>
      <button class="eq-button" {disabled} onclick={newProfile}>{t.equalizer.newProfile}</button>
    </div>
  </div>

  {#if importing}
    <div class="setting-row eq-import">
      <p class="hint">{t.equalizer.importHint}</p>
      <textarea class="eq-input mono" rows="6" placeholder={t.equalizer.importPlaceholder} bind:value={importText}></textarea>
      <label class="field">
        <span class="hint">{t.equalizer.name}</span>
        <input class="eq-input" maxlength="80" placeholder={t.equalizer.namePlaceholder} bind:value={importName} />
      </label>
      <div class="actions">
        <label class="eq-button file">
          {t.equalizer.chooseFile}
          <input type="file" accept=".txt,text/plain" onchange={pickFile} />
        </label>
        <button class="eq-button primary" disabled={busy || !importText.trim()} onclick={confirmImport}>
          {t.equalizer.confirmImport}
        </button>
        <button class="eq-button" onclick={() => (importing = false)}>{t.equalizer.cancel}</button>
      </div>
    </div>
  {/if}

  {#if active}
    <div class="setting-row eq-profile">
      <svg class="eq-curve" viewBox="0 0 {W} {H}" role="img" aria-label={t.equalizer.curve} preserveAspectRatio="none">
        <line class="eq-zero" x1="0" x2={W} y1={y(0)} y2={y(0)} />
        {#each GRID_HZ as hz (hz)}
          <line class="eq-grid" x1={x(hz)} x2={x(hz)} y1="0" y2={H} />
        {/each}
        <path class="eq-line" d={curve} />
      </svg>

      <div class="eq-head">
        <label class="field grow">
          <span class="hint">{t.equalizer.name}</span>
          <input class="eq-input" maxlength="80" value={active.name} {disabled} onchange={rename} />
        </label>
        <label class="field">
          <span class="hint">{t.equalizer.preamp}</span>
          <input
            class="eq-input mono num"
            type="number"
            step="0.1"
            min="-20"
            max="20"
            value={active.preamp ?? ""}
            placeholder={num(effectivePreamp(active), { maximumFractionDigits: 1 })}
            {disabled}
            onchange={setPreamp}
          />
        </label>
        <label class="eq-check">
          <input
            type="checkbox"
            checked={active.preamp === null}
            {disabled}
            onchange={(e) =>
              updateActive({
                preamp: (e.currentTarget as HTMLInputElement).checked ? null : effectivePreamp(active),
              })}
          />
          {t.equalizer.preampAuto}
        </label>
      </div>
      {#if active.preamp === null}
        <p class="hint">{t.equalizer.preampAutoHint(gainLabel(effectivePreamp(active)))}</p>
      {/if}

      <table class="eq-filters">
        <thead>
          <tr>
            <th>{t.equalizer.kind}</th>
            <th>{t.equalizer.freq}</th>
            <th>{t.equalizer.gain}</th>
            <th>{t.equalizer.q}</th>
            <th><span class="sr-only">{t.equalizer.removeFilter}</span></th>
          </tr>
        </thead>
        <tbody>
          {#each active.filters as f, i (i)}
            <tr>
              <td>
                <select
                  class="eq-select"
                  value={f.kind}
                  {disabled}
                  aria-label={t.equalizer.kind}
                  onchange={(e) => updateFilter(i, { kind: (e.currentTarget as HTMLSelectElement).value as EqFilterKind })}
                >
                  {#each KINDS as kind (kind)}
                    <option value={kind}>{t.equalizer.kinds[kind]}</option>
                  {/each}
                </select>
              </td>
              <td>
                <input class="eq-input mono num" type="number" min="20" max="20000" step="1" value={f.freq} {disabled}
                  aria-label={t.equalizer.freq} onchange={(e) => setNumber(i, "freq", e)} />
              </td>
              <td>
                <input class="eq-input mono num" type="number" min="-20" max="20" step="0.1" value={f.gain} {disabled}
                  aria-label={t.equalizer.gain} onchange={(e) => setNumber(i, "gain", e)} />
              </td>
              <td>
                <input class="eq-input mono num" type="number" min="0.1" max="10" step="0.01" value={f.q} {disabled}
                  aria-label={t.equalizer.q} onchange={(e) => setNumber(i, "q", e)} />
              </td>
              <td>
                <button class="eq-remove" title={t.equalizer.removeFilter} aria-label={t.equalizer.removeFilter} {disabled}
                  onclick={() => updateActive({ filters: active.filters.filter((_, j) => j !== i) })}>×</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if !active.filters.length}
        <p class="hint">{t.equalizer.noFilter}</p>
      {/if}
      <div class="actions">
        <button class="eq-button" disabled={disabled || active.filters.length >= MAX_FILTERS} onclick={addFilter}>
          {t.equalizer.addFilter}
        </button>
        <button class="eq-button danger" {disabled} onclick={deleteProfile}>{t.equalizer.deleteProfile}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .eq-settings {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 24px;
    max-width: 640px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .eq-import,
  .eq-profile {
    flex-direction: column;
    align-items: stretch;
  }

  .text,
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .grow {
    flex: 1;
  }

  .setting-label {
    font-weight: 500;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .eq-toolbar,
  .eq-head {
    display: flex;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 10px 16px;
    max-width: 640px;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .eq-input,
  .eq-select {
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text);
    background: var(--bg-hover);
  }

  textarea.eq-input {
    width: 100%;
    resize: vertical;
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .num {
    width: 92px;
  }

  .eq-check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding-bottom: 6px;
    font-size: 13px;
  }

  .eq-button {
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
  }

  .eq-button:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .eq-button.primary {
    border-color: transparent;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .eq-button.danger {
    color: var(--danger);
  }

  .eq-button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .file input {
    display: none;
  }

  .eq-curve {
    width: 100%;
    height: 140px;
    border-radius: var(--radius);
    background: var(--bg-hover);
  }

  .eq-zero,
  .eq-grid {
    stroke: var(--border);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .eq-grid {
    stroke-dasharray: 3 4;
  }

  .eq-line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }

  .eq-filters {
    border-collapse: collapse;
    font-size: 13px;
  }

  .eq-filters th {
    padding: 0 8px 6px 0;
    font-size: 11px;
    font-weight: 600;
    text-align: left;
    color: var(--text-muted);
  }

  .eq-filters td {
    padding: 3px 8px 3px 0;
  }

  .eq-remove {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    font-size: 16px;
    color: var(--text-muted);
  }

  .eq-remove:hover:not(:disabled) {
    color: var(--danger);
    background: var(--bg-hover);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
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

  .setting-switch:disabled {
    opacity: 0.6;
  }
</style>
