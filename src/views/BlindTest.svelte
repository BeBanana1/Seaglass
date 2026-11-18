<script lang="ts">
  // Blind test (#434) : réglages de la partie, manche en cours, révélation et score. La
  // partie (tirage, comparaison des réponses, score) est tenue par le backend : cette page
  // affiche `blind-test` et envoie les réponses.
  import { tick } from "svelte";
  import { commands, type BlindAnswer, type BlindMode, type BlindRoundResult } from "../lib/ipc";
  import { artistNames, coverLarge, coverSmall, fullTitle } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { blindTest } from "../lib/stores/blindTest.svelte";
  import { followPosition, player } from "../lib/stores/player.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import Icon from "../components/Icon.svelte";
  import LikeButton from "../components/LikeButton.svelte";
  import BlindScoreboard from "../components/BlindScoreboard.svelte";
  import { jam } from "../lib/stores/jam.svelte";

  const ROUND_OPTIONS = [5, 10, 20];

  let mode = $state<BlindMode>("typing");
  let rounds = $state(10);
  /** À plusieurs, par la Jam (#441). */
  let withJam = $state(true);
  let hostPlays = $state(true);
  /** Secondes avant la révélation automatique (à plusieurs). */
  let revealIn = $state(0);
  let text = $state("");
  let busy = $state(false);
  let input = $state<HTMLInputElement | null>(null);
  /** Avancement de l'extrait, de 0 à 1, interpolé entre deux `player-state`. */
  let clip = $state(0);

  const view = $derived(blindTest.view);
  const pool = $derived(blindTest.pool);
  const result = $derived<BlindRoundResult | null>(
    view?.phase === "revealed" ? (view.results.at(-1) ?? null) : null,
  );
  const lastRound = $derived(!!view && view.round >= view.rounds);
  /** Extrait pas encore lancé par le moteur (#445) : l'anneau attend au départ. */
  const clipLoading = $derived(
    view?.phase === "guess" && (player.state.loading || !view.clipPlaying),
  );
  const clipOver = $derived(
    view?.phase === "guess" && !clipLoading && clip >= 1 && player.state.paused,
  );
  const multi = $derived(!!view?.party.jam);
  const hostPlaying = $derived(!view || view.party.hostPlays);
  /** L'hôte peut encore répondre à la manche. */
  const hostCanAnswer = $derived(
    !!view && hostPlaying && !view.players.find((p) => p.host)?.answered,
  );
  const jamRunning = $derived(!!jam.status);
  const roundChoices = $derived(
    pool ? ROUND_OPTIONS.filter((n, i) => i === 0 || n <= pool.trackIds.length) : [],
  );

  async function run(action: () => Promise<unknown>) {
    if (busy) return;
    busy = true;
    try {
      await action();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    } finally {
      busy = false;
    }
  }

  /** Nouvelle partie ; « Rejouer » reprend le mode et le nombre de manches de la dernière. */
  function start(again = false) {
    if (!pool) return;
    if (again && view) {
      mode = view.mode;
      rounds = view.rounds;
      withJam = view.party.jam;
      hostPlays = view.party.hostPlays;
    }
    const count = Math.min(rounds, pool.trackIds.length);
    const together = withJam && jamRunning;
    run(() =>
      commands.blindTestStart(
        pool.trackIds,
        mode,
        count,
        pool.title,
        together,
        together ? hostPlays : true,
      ),
    );
  }

  function answer(a: BlindAnswer) {
    run(() => commands.blindTestAnswer(a));
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (text.trim()) answer({ kind: "text", text });
  }

  // Nouvelle manche : saisie vidée et prête.
  let lastRoundShown = 0;
  $effect(() => {
    const round = view?.phase === "guess" ? view.round : 0;
    if (round && round !== lastRoundShown) {
      lastRoundShown = round;
      text = "";
      tick().then(() => input?.focus());
    } else if (!round) lastRoundShown = 0;
  });

  // Compte à rebours de la révélation automatique, à plusieurs.
  $effect(() => {
    const at = view?.phase === "guess" ? view.revealAt : null;
    if (!at) {
      revealIn = 0;
      return;
    }
    const update = () => (revealIn = Math.max(0, Math.ceil((at - Date.now()) / 1000)));
    update();
    const timer = setInterval(update, 1000);
    return () => clearInterval(timer);
  });

  // Barre de l'extrait : position interpolée comme la barre de lecture.
  $effect(() => {
    if (view?.phase !== "guess") return;
    const { clipStart, clipLength } = view;
    if (clipLoading) {
      clip = 0;
      return;
    }
    return followPosition((p) => (clip = Math.min(1, Math.max(0, (p - clipStart) / clipLength))));
  });

  function verdict(r: BlindRoundResult): string {
    if (r.correct !== null)
      return r.title
        ? t.blindTest.right
        : r.chosen === null
          ? t.blindTest.passed
          : t.blindTest.missed;
    if (r.title && r.artist) return t.blindTest.both;
    if (r.title) return t.blindTest.titleOnly;
    if (r.artist) return t.blindTest.artistOnly;
    return r.answer === null ? t.blindTest.passed : t.blindTest.missed;
  }
</script>

<section class="blind-test">
  <header>
    <span class="eyebrow"><Icon name="quiz" size={14} /> {t.blindTest.title}</span>
    <h1>{view?.title || pool?.title || t.blindTest.title}</h1>
    {#if view}
      <div class="stats">
        <span>{t.blindTest.round(view.round, view.rounds)}</span>
        {#if hostPlaying}<span class="score">
            {t.blindTest.score(view.score, view.maxScore)}
          </span>{/if}
        {#if hostPlaying && view.streak > 1}<span class="streak">
            {t.blindTest.streak(view.streak)}
          </span>{/if}
        <button class="quit" onclick={() => run(() => commands.blindTestQuit())}>
          {t.blindTest.quit}
        </button>
      </div>
    {/if}
  </header>

  {#if !view}
    {#if pool}
      <div class="setup">
        <p class="hint">{t.blindTest.setup(t.common.tracks(pool.trackIds.length), pool.title)}</p>
        <fieldset>
          <legend>{t.blindTest.mode}</legend>
          <div class="segmented">
            <button
              class:on={mode === "typing"}
              aria-pressed={mode === "typing"}
              onclick={() => (mode = "typing")}
            >
              {t.blindTest.typing}
            </button>
            <button
              class:on={mode === "choices"}
              aria-pressed={mode === "choices"}
              onclick={() => (mode = "choices")}
            >
              {t.blindTest.choices}
            </button>
          </div>
          <p class="hint small">
            {mode === "typing" ? t.blindTest.typingHint : t.blindTest.choicesHint}
          </p>
        </fieldset>
        <fieldset>
          <legend>{t.blindTest.rounds}</legend>
          <div class="segmented">
            {#each roundChoices as n (n)}
              <button
                class:on={rounds === n}
                aria-pressed={rounds === n}
                onclick={() => (rounds = n)}
              >
                {Math.min(n, pool.trackIds.length)}
              </button>
            {/each}
          </div>
        </fieldset>
        {#if jamRunning}
          <fieldset class="together">
            <label class="check">
              <input type="checkbox" bind:checked={withJam} />
              {t.blindTest.withJam}
            </label>
            {#if withJam}
              <label class="check">
                <input type="checkbox" bind:checked={hostPlays} />
                {t.blindTest.hostPlays}
              </label>
              <p class="hint small">{t.blindTest.withJamHint}</p>
            {/if}
          </fieldset>
        {:else}
          <p class="hint small">{t.blindTest.noJam}</p>
        {/if}
        <button class="primary" disabled={busy} onclick={() => start()}>
          <Icon name="play" size={16} />
          {t.blindTest.play}
        </button>
        <p class="hint small">{t.blindTest.queueKept}</p>
      </div>
    {:else}
      <p class="hint">{t.blindTest.noPool}</p>
    {/if}
  {:else if view.phase === "guess"}
    <div class="round">
      <div class="disc" class:spinning={player.advancing && !clipLoading} style:--clip={clip}>
        <span class="mystery">?</span>
      </div>
      <p class="status" aria-live="polite">
        {clipLoading
          ? t.blindTest.loading
          : clipOver
            ? t.blindTest.clipOver
            : t.blindTest.listening}
      </p>
      <div class="clip-bar" aria-hidden="true"><span style:width="{clip * 100}%"></span></div>
      <button class="secondary" onclick={() => run(() => commands.blindTestReplay())}>
        <Icon name="repeat" size={16} />
        {t.blindTest.replay}
      </button>

      {#if !hostCanAnswer}
        <p class="status">{hostPlaying ? t.blindTest.waiting : t.blindTest.hosting}</p>
      {:else if view.mode === "typing"}
        <form class="answer" onsubmit={submit}>
          <input
            bind:this={input}
            bind:value={text}
            type="text"
            autocomplete="off"
            spellcheck="false"
            maxlength="200"
            placeholder={t.blindTest.placeholder}
            aria-label={t.blindTest.placeholder}
          />
          <button class="primary" type="submit" disabled={busy || !text.trim()}>
            {t.blindTest.answer}
          </button>
        </form>
      {:else}
        <div class="choices">
          {#each view.choices as choice, i (i)}
            <button
              class="choice"
              disabled={busy}
              onclick={() => answer({ kind: "choice", index: i })}
            >
              <span class="choice-title">{choice.title}</span>
              <span class="choice-artist">{choice.artist}</span>
            </button>
          {/each}
        </div>
      {/if}
      {#if hostCanAnswer}
        <button class="link pass" disabled={busy} onclick={() => answer({ kind: "pass" })}>
          {t.blindTest.pass}
        </button>
      {/if}
      {#if multi}
        <button
          class="primary"
          disabled={busy}
          onclick={() => run(() => commands.blindTestReveal())}
        >
          {t.blindTest.reveal}
        </button>
        {#if revealIn > 0}<p class="hint small">{t.blindTest.revealIn(revealIn)}</p>{/if}
        <BlindScoreboard players={view.players} showAnswered />
      {/if}
    </div>
  {:else if view.phase === "revealed" && result}
    <div class="round revealed">
      <div class="cover">
        {#if coverLarge(result.track.album)}<img src={coverLarge(result.track.album)} alt="" />{/if}
      </div>
      {#if hostPlaying}
        <p class="verdict" class:good={result.points > 0}>
          {verdict(result)} <span class="points">{t.blindTest.points(result.points)}</span>
        </p>
      {/if}
      <div class="answer-track">
        <span class="track-title" class:found={result.title}>{fullTitle(result.track)}</span>
        <span class="track-artist" class:found={result.artist}>{artistNames(result.track)}</span>
        <LikeButton track={result.track} />
      </div>
      {#if result.answer}<p class="hint small">{t.blindTest.yourAnswer(result.answer)}</p>{/if}
      {#if result.correct !== null && result.chosen !== null && result.chosen !== result.correct}
        <p class="hint small">
          {t.blindTest.yourAnswer(`${view.choices[result.chosen]?.title ?? ""}`)}
        </p>
      {/if}
      <button class="primary" disabled={busy} onclick={() => run(() => commands.blindTestNext())}>
        {lastRound ? t.blindTest.seeScore : t.blindTest.next}
        <Icon name="next" size={16} />
      </button>
      {#if multi}<BlindScoreboard players={view.players} round={result.players} />{/if}
    </div>
  {:else if view.phase === "finished"}
    <div class="summary">
      <h2>{t.blindTest.finished}</h2>
      {#if hostPlaying}<p class="final">{t.blindTest.score(view.score, view.maxScore)}</p>{/if}
      {#if multi}<BlindScoreboard players={view.players} />{/if}
      {#if hostPlaying && view.bestStreak > 1}<p class="hint">
          {t.blindTest.bestStreak(view.bestStreak)}
        </p>{/if}
      <ol class="results">
        {#each view.results as r, i (i)}
          <li class:good={r.points > 0}>
            {#if coverSmall(r.track.album)}<img
                src={coverSmall(r.track.album)}
                alt=""
              />{:else}<span class="thumb"></span>{/if}
            <span class="result-meta">
              <span class="track-title">{fullTitle(r.track)}</span>
              <span class="track-artist">{artistNames(r.track)}</span>
            </span>
            {#if hostPlaying}
              <span class="result-verdict">{verdict(r)}</span>
              <span class="points">{t.blindTest.points(r.points)}</span>
            {/if}
          </li>
        {/each}
      </ol>
      <div class="actions">
        {#if pool}<button class="primary" disabled={busy} onclick={() => start(true)}>
            <Icon name="play" size={16} />
            {t.blindTest.again}
          </button>{/if}
        <button
          class="secondary"
          disabled={busy}
          onclick={() => run(() => commands.blindTestQuit())}
        >
          {t.blindTest.quit}
        </button>
      </div>
    </div>
  {/if}
</section>

<style>
  section {
    padding: 24px;
    max-width: 760px;
    margin: 0 auto;
  }

  header {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 24px;
  }

  .eyebrow {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  h1 {
    margin: 0;
    font-size: 30px;
    font-weight: 700;
    line-height: 1.15;
  }

  .stats {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 16px;
    color: var(--text-muted);
  }

  .score {
    color: var(--text);
    font-weight: 600;
  }

  .streak {
    color: var(--accent);
    font-weight: 600;
  }

  .quit {
    margin-left: auto;
    font-size: 13px;
    color: var(--text-muted);
  }

  .quit:hover {
    color: var(--text);
  }

  .hint {
    margin: 0;
    color: var(--text-muted);
    line-height: 1.5;
  }

  .hint.small {
    font-size: 13px;
  }

  .setup {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 20px;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    border: none;
  }

  legend {
    margin-bottom: 8px;
    font-weight: 600;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }

  .check input {
    accent-color: var(--accent);
  }

  .segmented {
    display: inline-flex;
    align-self: flex-start;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    overflow: hidden;
    clip-path: var(--control-clip);
  }

  .segmented button {
    padding: 8px 16px;
    color: var(--text-muted);
  }

  .segmented button + button {
    border-left: 1px solid var(--border);
  }

  .segmented button.on {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .primary,
  .secondary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 18px;
    border-radius: var(--radius-pill);
    font-weight: 600;
    clip-path: var(--control-clip);
  }

  .primary {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .primary:hover:not(:disabled) {
    background: var(--accent-strong);
  }

  .primary:disabled {
    opacity: 0.5;
  }

  .secondary {
    border: 1px solid var(--border);
  }

  .secondary:hover {
    background: var(--bg-hover);
  }

  .round :global(.blind-scoreboard) {
    max-width: 480px;
    margin-top: 12px;
    text-align: left;
  }

  .round {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    text-align: center;
  }

  /* Disque masqué : l'anneau suit l'avancement de l'extrait. */
  .disc {
    position: relative;
    display: grid;
    place-items: center;
    width: 168px;
    height: 168px;
    border-radius: 50%;
    background:
      radial-gradient(circle, var(--bg-raised) 0 58%, transparent 59%),
      conic-gradient(var(--accent) calc(var(--clip) * 360deg), var(--bg-hover) 0);
    box-shadow: var(--shadow-cover, 0 8px 24px rgb(0 0 0 / 0.4));
  }

  .mystery {
    font-size: 64px;
    font-weight: 700;
    color: var(--text-muted);
  }

  .disc.spinning .mystery {
    animation: pulse 1.6s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      transform: scale(1.08);
      color: var(--accent);
    }
  }

  .status {
    margin: 0;
    min-height: 1.4em;
    color: var(--text-muted);
  }

  .clip-bar {
    width: min(320px, 100%);
    height: 4px;
    border-radius: 2px;
    background: var(--bg-hover);
    overflow: hidden;
  }

  .clip-bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .answer {
    display: flex;
    gap: 8px;
    width: min(520px, 100%);
    margin-top: 8px;
  }

  .answer input {
    flex: 1;
    min-width: 0;
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--bg-raised);
    color: var(--text);
    font: inherit;
  }

  .answer input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .choices {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    width: min(620px, 100%);
    margin-top: 8px;
  }

  .choice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 12px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card, 10px);
    text-align: left;
  }

  .choice:hover:not(:disabled) {
    border-color: var(--accent);
    background: var(--bg-hover);
  }

  .choice-title,
  .track-title {
    font-weight: 600;
  }

  .choice-artist,
  .track-artist {
    font-size: 13px;
    color: var(--text-muted);
  }

  .choice-title,
  .choice-artist {
    max-width: 100%;
    overflow: hidden;
  }

  /* Titre sur 2 lignes au plus : sur une colonne étroite (Android), une
     seule ligne rognait les titres longs au point de les rendre illisibles (#446). */
  .choice-title {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow-wrap: anywhere;
  }

  .choice-artist {
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .link {
    color: var(--text-muted);
    text-decoration: underline;
  }

  .link:hover {
    color: var(--text);
  }

  .cover {
    width: 168px;
    height: 168px;
    border-radius: var(--radius-cover, 8px);
    overflow: hidden;
    background: var(--bg-raised);
    box-shadow: var(--shadow-cover, 0 8px 24px rgb(0 0 0 / 0.4));
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .verdict {
    margin: 0;
    font-size: 20px;
    font-weight: 700;
    color: var(--text-muted);
  }

  .verdict.good {
    color: var(--accent);
  }

  .points {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-muted);
  }

  .answer-track {
    display: grid;
    grid-template-columns: auto auto;
    align-items: center;
    column-gap: 10px;
  }

  .answer-track .track-title {
    font-size: 18px;
  }

  .answer-track .track-artist {
    grid-row: 2;
  }

  .answer-track :global(.like-button) {
    grid-row: 1 / 3;
    grid-column: 2;
  }

  .track-title.found,
  .track-artist.found {
    color: var(--accent);
  }

  .summary {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  h2 {
    margin: 0;
  }

  .final {
    margin: 0;
    font-size: 28px;
    font-weight: 700;
    color: var(--accent);
  }

  .results {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 8px 0;
    padding: 0;
    list-style: none;
  }

  .results li {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border-radius: var(--radius-card, 8px);
  }

  .results li.good .result-verdict {
    color: var(--accent);
  }

  .results img,
  .thumb {
    width: 40px;
    height: 40px;
    border-radius: 4px;
    object-fit: cover;
    background: var(--bg-raised);
  }

  .result-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .result-meta span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result-verdict {
    font-size: 13px;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    gap: 10px;
  }

  /* Android : disque plus petit, pour que les quatre propositions tiennent à l'écran. */
  :global(:root[data-platform="android"]) .disc,
  :global(:root[data-platform="android"]) .cover {
    width: 128px;
    height: 128px;
  }

  :global(:root[data-platform="android"]) .mystery {
    font-size: 48px;
  }

  :global(:root[data-platform="android"]) .choices {
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
  }

  :global(:root[data-platform="android"]) .choice {
    padding: 10px 14px;
  }

  :global(:root[data-platform="android"]) .results li {
    grid-template-columns: 40px minmax(0, 1fr) auto;
  }

  :global(:root[data-platform="android"]) .result-verdict {
    display: none;
  }
</style>
