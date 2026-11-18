<script lang="ts" module>
  // Clin d'œil (#399) : minuteur réglé sur une heure ici, partagé par la barre et « En lecture ».
  const chosen = $state<{ hour: boolean; setAt: number | null }>({ hour: false, setAt: null });
</script>

<script lang="ts">
  // Minuteur de mise en veille (#85) : bouton lune, menu des durées, temps restant affiché
  // tant qu'il est actif. L'échéance vient du backend (`player-state`), le compte à rebours
  // est calculé ici. Dans la barre de lecture et dans « En lecture ».
  import { commands, type SleepRequest } from "../lib/ipc";
  import { formatTime } from "../lib/format";
  import { player } from "../lib/stores/player.svelte";
  import { sleepScope } from "../lib/stores/queue.svelte";
  import { closeMenu, menu, openMenu, type MenuEntry } from "../lib/stores/menu.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { locale, t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import { ui } from "../lib/stores/ui.svelte";
  import { eggs } from "../lib/eggs/eggs.svelte";

  const DURATIONS = [15, 30, 45, 60, 90];

  let { size = 18 }: { size?: number } = $props();

  let button = $state<HTMLButtonElement>();
  let now = $state(Date.now());

  const sleep = $derived(player.state.sleep);
  const endsAt = $derived(sleep?.kind === "minutes" ? sleep.endsAt : null);

  $effect(() => {
    if (endsAt === null) return;
    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });

  const remaining = $derived(endsAt === null ? null : formatTime(Math.max(0, endsAt - now) / 1000));
  /** 21 octobre (#399) : le minuteur se lit comme un cadran à trois lignes. */
  const circuits = $derived(eggs.date === "timeCircuits" && endsAt !== null);
  const clock = (ms: number) =>
    new Date(ms).toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" });
  const label = $derived(
    circuits && endsAt !== null
      ? t.eggs.circuits(
          clock(endsAt),
          clock(now),
          chosen.setAt === null ? "--:--" : clock(chosen.setAt),
        )
      : remaining !== null
        ? t.player.sleepIn(remaining) +
          (chosen.hour && ui.settings.easterEggs ? t.eggs.sevenYears : "")
        : sleep?.kind === "endOfContext"
          ? t.player.sleepAtEndOfContext[sleepScope()]
          : sleep
            ? t.player.sleepAtEnd
            : t.player.sleepTimer,
  );

  async function set(timer: SleepRequest | null) {
    chosen.hour = timer?.kind === "minutes" && timer.minutes === 60;
    chosen.setAt = timer ? Date.now() : null;
    try {
      await commands.setSleepTimer(timer);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  function toggleMenu() {
    if (!button) return;
    if (menu.open?.from === button) return closeMenu();
    const p = t.player;
    const entries: MenuEntry[] = DURATIONS.map((minutes) => ({
      label: p.sleepMinutes(minutes),
      action: () => set({ kind: "minutes", minutes }),
    }));
    entries.push(
      { label: p.sleepEndOfTrack, action: () => set({ kind: "endOfTrack" }) },
      { label: p.sleepEndOfContext[sleepScope()], action: () => set({ kind: "endOfContext" }) },
    );
    if (sleep)
      entries.push("separator", { label: p.sleepOff, icon: "close", action: () => set(null) });
    const rect = button.getBoundingClientRect();
    // Dans la barre du bas, le menu s'ouvre au-dessus du bouton (recalé s'il déborde).
    const y = rect.top > innerHeight / 2 ? rect.top - 4 : rect.bottom + 4;
    openMenu({ x: rect.left, y, entries, from: button });
  }
</script>

<button
  class="sleep-timer"
  class:on={!!sleep}
  bind:this={button}
  onclick={toggleMenu}
  title={label}
  aria-label={label}
  aria-haspopup="menu"
  aria-expanded={menu.open?.from === button && !!button}
>
  <Icon name="moon" {size} />
  {#if remaining !== null}
    <span class="sleep-remaining" class:circuits>{remaining}</span>
  {:else if sleep?.kind === "endOfContext"}
    <span class="sleep-remaining">{t.player.sleepContextShort[sleepScope()]}</span>
  {:else if sleep}
    <span class="sleep-remaining">{t.player.sleepEndShort}</span>
  {/if}
</button>

<style>
  .sleep-timer {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 6px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .sleep-timer:hover {
    color: var(--text);
  }

  .sleep-timer.on {
    color: var(--accent);
  }

  .sleep-remaining {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  /* 21 octobre : afficheur à segments, chiffres orange sur fond sombre. */
  .sleep-remaining.circuits {
    padding: 1px 4px;
    border-radius: 2px;
    font-family: var(--font-mono);
    color: #ff8a2a;
    background: #1a0c04;
    text-shadow: 0 0 4px #ff6a00;
    letter-spacing: 0.08em;
  }
</style>
