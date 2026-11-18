<!-- Page Réglages : une section par onglet. Les valeurs viennent toutes du backend. -->
<script lang="ts">
  import {
    commands,
    events,
    type AudioDevice,
    type CacheSize,
    type CloseAction,
    type EqSettings,
    type HiresUpgrade,
    type MobileQuality,
    type LanguagePref,
    type Normalization,
    type PlaybackSettings,
  } from "../lib/ipc";
  import { replace, type SettingsSection } from "../lib/stores/nav.svelte";
  import { copyText, pushToast } from "../lib/stores/toasts.svelte";
  import { cancelLibraryExport, libraryExport, startLibraryExport } from "../lib/stores/libraryExport.svelte";
  import {
    MOTIONS,
    setCloseAction,
    setContentFilter,
    setDiscord,
    setEasterEggs,
    setGlobalShortcuts,
    setMotion,
    setUiToggle,
    setZoom,
    ui,
    ZOOM_LEVELS,
  } from "../lib/stores/ui.svelte";
  import { setLanguage, t } from "../lib/i18n.svelte";
  import ThemePicker from "../components/ThemePicker.svelte";
  import CurrentTheme from "../components/CurrentTheme.svelte";
  import ChangelogList from "../components/ChangelogList.svelte";
  import OutputDevicePicker from "../components/OutputDevicePicker.svelte";
  import { platform } from "../lib/platform.svelte";
  import { jam, setJamEnabled } from "../lib/stores/jam.svelte";
  import JamPanel from "../components/JamPanel.svelte";
  import RelaySettings from "../components/RelaySettings.svelte";
  import ListenBrainzSettings from "../components/ListenBrainzSettings.svelte";
  import EqualizerSettings from "../components/EqualizerSettings.svelte";
  import RemotePanel from "../components/RemotePanel.svelte";
  import LinkedComputerPanel from "../components/LinkedComputerPanel.svelte";
  import BlockedList from "../components/BlockedList.svelte";
  import CreditsRoll from "../components/CreditsRoll.svelte";
  import { playbackSettingsChanged } from "../lib/stores/palette.svelte";
  import GlobalShortcutsList from "../components/GlobalShortcutsList.svelte";

  let { section, onLogout }: { section: SettingsSection; onLogout?: () => void } = $props();

  // Android (#24) : ni sortie audio ni cache (réglages de mpv), qualité choisie parmi les
  // paliers de TIDAL (#366), ni fenêtre, démarrage auto ou mises à jour (réglages du bureau).
  const desktop = $derived(!platform.mobile);

  const sections: SettingsSection[] = ["appearance", "playback", "app"];

  const LANGUAGES: LanguagePref[] = ["system", "fr", "en"];

  const CLOSE_ACTIONS: CloseAction[] = ["quit", "tray"];

  const CACHE_SIZES: CacheSize[] = [32, 64, 128, 256];

  const HIRES_MODES: HiresUpgrade[] = ["auto", "ask", "off"];

  const MOBILE_QUALITIES: MobileQuality[] = ["low", "high", "lossless", "max"];
  type NetworkQualityKey = "wifiQuality" | "cellularQuality";
  const NETWORKS: NetworkQualityKey[] = ["wifiQuality", "cellularQuality"];
  const NORMALIZATION_MODES: Normalization[] = ["off", "track", "album"];
  let playback = $state<PlaybackSettings | null>(null);
  const cacheSize = $derived(playback?.cacheSize ?? null);
  const hiresUpgrade = $derived(playback?.hiresUpgrade ?? null);
  const normalization = $derived(playback?.normalization ?? null);
  /** Mode exclusif (#245) : la normalisation est suspendue, ses choix grisés. */
  const exclusive = $derived(playback?.exclusive === true);
  let autostart = $state<boolean | null>(null);
  let autostartBusy = $state(false);
  let discordAvailable = $state(false);
  // Raccourcis globaux (#413) : le système les offre (pas sous Linux sans serveur X).
  let globalShortcutsAvailable = $state(false);
  let changelogOpen = $state(false);
  /** Clin d'œil (#399) : 7 clics de suite sur la version lancent le générique de fin. */
  let creditsOpen = $state(false);
  let versionClicks = 0;
  let versionTimer: ReturnType<typeof setTimeout> | undefined;

  function clickVersion() {
    clearTimeout(versionTimer);
    versionTimer = setTimeout(() => (versionClicks = 0), 1500);
    if (++versionClicks < 7) return;
    versionClicks = 0;
    if (ui.settings.easterEggs) creditsOpen = true;
  }
  // Faux sous Linux hors d'une AppImage (#314) : pas de bouton « Vérifier ».
  let updatesAvailable = $state(true);
  // Dossier des mises à jour compilé (#452) ; `null` : mises à jour absentes, rien à proposer.
  let updateServer = $state<string | null>(null);
  let exclusiveAvailable = $state(false);
  // Android 12+ (#203) : liens TIDAL donnés à l'app ; `null` = rien à régler ou pas encore lu.
  let links = $state<{ enabled: boolean } | null>(null);

  // Mode exclusif changé depuis la palette (#412) : relu, sinon le prochain réglage
  // enregistré ici remettrait l'ancienne valeur.
  $effect(() => {
    if (playbackSettingsChanged.count > 0) void loadPlayback();
  });

  // Profil d'un périphérique de sortie appliqué par le backend (#416) : normalisation et
  // mode exclusif ont pu changer.
  $effect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    void events.onPlaybackSettings(() => void loadPlayback()).then((u) => {
      if (alive) unlisten = u;
      else u();
    });
    return () => {
      alive = false;
      unlisten?.();
    };
  });

  $effect(() => {
    if (section === "playback" && playback === null) loadPlayback();
    if (section === "app" && autostart === null && desktop) loadAutostart();
  });

  $effect(() => {
    if (section === "playback" && desktop) {
      commands.exclusiveAvailable().then((v) => (exclusiveAvailable = v), fail);
    }
  });

  $effect(() => {
    if (section === "app" && desktop) {
      commands.discordAvailable().then((v) => (discordAvailable = v), fail);
      commands.globalShortcutsStatus().then((s) => (globalShortcutsAvailable = s.available), fail);
      commands.updatesAvailable().then((v) => (updatesAvailable = v), fail);
    }
  });

  $effect(() => {
    if (section === "app") commands.updateServer().then((v) => (updateServer = v), fail);
  });

  $effect(() => {
    if (section !== "app" || desktop) return;
    const load = () => commands.linkHandling().then((v) => (links = v), fail);
    load();
    // Retour depuis le réglage du système : l'état a pu changer.
    const onVisible = () => document.visibilityState === "visible" && load();
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  function openLinkSettings() {
    commands.openLinkSettings().catch(fail);
  }

  async function toggleDiscord() {
    try {
      await setDiscord(!ui.settings.discord);
    } catch (e) {
      fail(e);
    }
  }

  async function toggleGlobalShortcuts() {
    try {
      const current = ui.settings.globalShortcuts;
      await setGlobalShortcuts({ ...current, enabled: !current.enabled });
    } catch (e) {
      fail(e);
    }
  }

  async function toggleHideExplicit() {
    try {
      await setContentFilter({ ...ui.settings.content, hideExplicit: !ui.settings.content.hideExplicit });
    } catch (e) {
      fail(e);
    }
  }

  async function toggleHideAi() {
    try {
      await setContentFilter({ ...ui.settings.content, hideAi: !ui.settings.content.hideAi });
    } catch (e) {
      fail(e);
    }
  }

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  async function loadPlayback() {
    try {
      playback = await commands.playbackSettings();
    } catch (e) {
      fail(e);
    }
  }

  async function savePlayback(next: PlaybackSettings) {
    try {
      await commands.setPlaybackSettings(next);
      playback = next;
    } catch (e) {
      fail(e);
    }
  }

  async function chooseLanguage(pref: LanguagePref) {
    if (pref === ui.settings.language) return;
    try {
      await setLanguage(pref);
      ui.settings = { ...ui.settings, language: pref };
    } catch (e) {
      fail(e);
    }
  }

  async function chooseCloseAction(action: CloseAction) {
    if (action === ui.settings.closeAction) return;
    try {
      await setCloseAction(action);
    } catch (e) {
      fail(e);
    }
  }

  function setCacheSize(size: CacheSize) {
    if (playback && size !== playback.cacheSize) savePlayback({ ...playback, cacheSize: size });
  }

  function setNormalization(mode: Normalization) {
    if (playback && mode !== playback.normalization) savePlayback({ ...playback, normalization: mode });
  }

  function setEqualizer(equalizer: EqSettings) {
    if (playback) savePlayback({ ...playback, equalizer });
  }

  function setAudioDevice(device: AudioDevice | null) {
    if (playback) savePlayback({ ...playback, audioDevice: device });
  }

  function setHiresUpgrade(mode: HiresUpgrade) {
    if (playback && mode !== playback.hiresUpgrade) savePlayback({ ...playback, hiresUpgrade: mode });
  }

  /** Qualité d'un réseau (#365) : `wifiQuality` (non facturé) ou `cellularQuality`. */
  function setNetworkQuality(network: NetworkQualityKey, quality: MobileQuality) {
    if (playback && quality !== playback[network]) savePlayback({ ...playback, [network]: quality });
  }

  function toggleLowBatteryQuality() {
    if (playback) savePlayback({ ...playback, lowBatteryQuality: !playback.lowBatteryQuality });
  }

  function toggleExclusive() {
    if (playback) savePlayback({ ...playback, exclusive: !playback.exclusive });
  }

  function toggleDeviceProfiles() {
    if (playback) savePlayback({ ...playback, deviceProfiles: !playback.deviceProfiles });
  }

  function togglePauseOnUnplug() {
    if (playback) savePlayback({ ...playback, pauseOnUnplug: !playback.pauseOnUnplug });
  }

  function toggleResumeOnLaunch() {
    if (playback) savePlayback({ ...playback, resumeOnLaunch: !playback.resumeOnLaunch });
  }

  function toggleAutoRadio() {
    if (playback) savePlayback({ ...playback, autoRadio: !playback.autoRadio });
  }

  async function loadAutostart() {
    try {
      autostart = await commands.autostartEnabled();
    } catch (e) {
      fail(e);
    }
  }

  let updateCheck = $state<"idle" | "checking" | "uptodate" | "downloading" | "ready">("idle");
  let downloadingVersion = $state("");

  // Bureau (#187) : la vérification répond avant la fin du téléchargement de l'installeur.
  $effect(() => {
    const ready = events.onUpdateReady(() => {
      if (updateCheck === "downloading") updateCheck = "ready";
    });
    const failed = events.onUpdateFailed((message) => {
      if (updateCheck !== "downloading") return;
      updateCheck = "idle";
      fail(t.settings.checkFailed(message));
    });
    return () => {
      void ready.then((f) => f());
      void failed.then((f) => f());
    };
  });

  async function checkUpdates() {
    if (updateCheck === "checking" || updateCheck === "downloading") return;
    updateCheck = "checking";
    try {
      // Une version prête est réémise en `update-ready` : UpdatePrompt la propose.
      const info = await commands.checkForUpdates();
      if (info?.downloading) downloadingVersion = info.version;
      updateCheck = !info ? "uptodate" : info.downloading ? "downloading" : "ready";
    } catch (e) {
      updateCheck = "idle";
      fail(t.settings.checkFailed(String(e)));
    }
  }

  async function clearPlayHistory() {
    try {
      await commands.clearPlayHistory();
      pushToast({ level: "info", message: t.settings.playHistoryCleared });
    } catch (e) {
      fail(e);
    }
  }

  function copyLog() {
    void copyText(commands.currentLog, t.settings.logCopied, t.settings.logUnavailable);
  }

  // #352 : sauvegarde des réglages par le presse-papiers ; `null` = zone d'import fermée.
  let importText = $state<string | null>(null);
  let importing = $state(false);

  function copySettings() {
    void copyText(commands.exportSettings, t.settings.settingsCopied, t.settings.logUnavailable);
  }

  async function applyImport() {
    if (importText === null || !importText.trim() || importing) return;
    importing = true;
    try {
      const report = await commands.importSettings(importText);
      pushToast({ level: "info", message: t.settings.settingsImported(report.applied.length, report.ignored) });
      importText = null;
      // Tout relire depuis le backend : thème, langue, épingles, réglages de lecture.
      setTimeout(() => location.reload(), 1500);
    } catch (e) {
      fail(e);
    } finally {
      importing = false;
    }
  }

  function copyCrash() {
    void copyText(commands.lastCrashReport, t.settings.crashCopied, t.settings.noCrash);
  }

  async function openProject(page: "repo" | "newIssue") {
    try {
      await commands.openProjectPage(page);
    } catch (e) {
      fail(e);
    }
  }

  async function toggleAutostart() {
    if (autostart === null || autostartBusy) return;
    autostartBusy = true;
    try {
      autostart = await commands.setAutostart(!autostart);
    } catch (e) {
      fail(e);
      await loadAutostart();
    } finally {
      autostartBusy = false;
    }
  }
</script>

<section class="settings">
  <h1>{t.settings.title}</h1>
  <div class="tabs" role="tablist">
    {#each sections as id (id)}
      <button
        role="tab"
        aria-selected={id === section}
        class:active={id === section}
        onclick={() => replace({ name: "settings", section: id })}
      >
        {t.settings.sections[id]}
      </button>
    {/each}
  </div>

  {#if section === "appearance"}
    <div class="group">
      <h2>{t.settings.language}</h2>
      <p class="hint">{t.settings.languageHint}</p>
      <div class="cache-sizes languages" role="radiogroup" aria-label={t.settings.language}>
        {#each LANGUAGES as lang (lang)}
          <button
            class="cache-size language"
            class:selected={ui.settings.language === lang}
            role="radio"
            aria-checked={ui.settings.language === lang}
            lang={lang === "system" ? undefined : lang}
            onclick={() => chooseLanguage(lang)}
          >
            {t.settings.languages[lang]}
          </button>
        {/each}
      </div>
    </div>
    <div class="group">
      <h2>{t.settings.theme}</h2>
      <p class="hint">{t.settings.themeHint}</p>
      <CurrentTheme />
    </div>
    <div class="group">
      <h2>{t.settings.motion}</h2>
      <p class="hint">{t.settings.motionHint}</p>
      <div class="cache-sizes motions" role="radiogroup" aria-label={t.settings.motion}>
        {#each MOTIONS as motion (motion)}
          <button
            class="cache-size motion"
            class:selected={ui.settings.motion === motion}
            role="radio"
            aria-checked={ui.settings.motion === motion}
            onclick={() => setMotion(motion).catch(fail)}
          >
            {t.settings.motions[motion]}
          </button>
        {/each}
      </div>
    </div>
    <!-- #399 : clins d'œil (dates, recherches), activés par défaut. -->
    <div class="group">
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.eggs}</span>
          <span class="hint">{t.settings.eggsHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.easterEggs}
          role="switch"
          aria-checked={ui.settings.easterEggs}
          aria-label={t.settings.eggs}
          onclick={() => setEasterEggs(!ui.settings.easterEggs).catch(fail)}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
    {#if desktop}
    <div class="group">
      <h2>{t.settings.zoom}</h2>
      <p class="hint">{t.settings.zoomHint}</p>
      <div class="cache-sizes" role="radiogroup" aria-label={t.settings.zoom}>
        {#each ZOOM_LEVELS as level (level)}
          <button
            class="cache-size"
            class:selected={ui.settings.zoom === level}
            role="radio"
            aria-checked={ui.settings.zoom === level}
            onclick={() => setZoom(level).catch(fail)}
          >
            {level} %
          </button>
        {/each}
      </div>
    </div>
    {/if}
    <!-- Galerie en dernier (#390) : plus aucun réglage caché sous elle. -->
    <div class="group">
      <h2>{t.settings.themeGallery}</h2>
      <ThemePicker />
    </div>
  {:else if section === "playback"}
    {#if desktop}
    <div class="group">
      <h2>{t.settings.outputDevice}</h2>
      <p class="hint">{t.settings.outputDeviceHint}</p>
      <OutputDevicePicker value={playback ? playback.audioDevice : undefined} onchange={setAudioDevice} />
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.pauseOnUnplug}</span>
          <span class="hint">{t.settings.pauseOnUnplugHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={playback?.pauseOnUnplug === true}
          role="switch"
          aria-checked={playback?.pauseOnUnplug === true}
          aria-label={t.settings.pauseOnUnplug}
          disabled={playback === null}
          onclick={togglePauseOnUnplug}
        >
          <span class="knob"></span>
        </button>
      </label>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.deviceProfiles}</span>
          <span class="hint">{t.settings.deviceProfilesHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={playback?.deviceProfiles === true}
          role="switch"
          aria-checked={playback?.deviceProfiles === true}
          aria-label={t.settings.deviceProfiles}
          disabled={playback === null}
          onclick={toggleDeviceProfiles}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
    {#if exclusiveAvailable}
    <div class="group">
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.exclusive}</span>
          <span class="hint">{t.settings.exclusiveHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={exclusive}
          role="switch"
          aria-checked={exclusive}
          aria-label={t.settings.exclusive}
          disabled={playback === null}
          onclick={toggleExclusive}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
    {/if}
    <div class="group">
      <h2>{t.settings.cache}</h2>
      <p class="hint">{t.settings.cacheHint}</p>
      <div class="cache-sizes" role="radiogroup" aria-label={t.settings.cacheSize}>
        {#each CACHE_SIZES as size (size)}
          <button
            class="cache-size"
            class:selected={cacheSize === size}
            role="radio"
            aria-checked={cacheSize === size}
            disabled={cacheSize === null}
            onclick={() => setCacheSize(size)}
          >
            {t.settings.megabytes(size)}
          </button>
        {/each}
      </div>
      <p class="note">{t.settings.cacheNote}</p>
    </div>
    <div class="group">
      <h2>{t.settings.quality}</h2>
      <p class="hint">{t.settings.qualityHint}</p>
      <div class="cache-sizes hires-modes" role="radiogroup" aria-label={t.settings.hiresEdition}>
        {#each HIRES_MODES as mode (mode)}
          <button
            class="cache-size hires-mode"
            class:selected={hiresUpgrade === mode}
            role="radio"
            aria-checked={hiresUpgrade === mode}
            disabled={hiresUpgrade === null}
            onclick={() => setHiresUpgrade(mode)}
          >
            {t.settings.hiresModes[mode].label}
          </button>
        {/each}
      </div>
      <p class="note">{hiresUpgrade ? t.settings.hiresModes[hiresUpgrade].note : ""}</p>
    </div>
    {:else}
    <div class="group">
      <h2>{t.settings.quality}</h2>
      <p class="hint">{t.settings.mobileQualityHint}</p>
      {#each NETWORKS as network (network)}
        <h3 class="network-quality">{t.settings.networks[network]}</h3>
        <div class="cache-sizes mobile-qualities" role="radiogroup" aria-label={t.settings.networks[network]}>
          {#each MOBILE_QUALITIES as quality (quality)}
            <button
              class="cache-size hires-mode"
              class:selected={playback?.[network] === quality}
              role="radio"
              aria-checked={playback?.[network] === quality}
              disabled={playback === null}
              onclick={() => setNetworkQuality(network, quality)}
            >
              {t.settings.mobileQualities[quality].label}
            </button>
          {/each}
        </div>
        <p class="note">{playback ? t.settings.mobileQualities[playback[network]].note : ""}</p>
      {/each}
      <label class="setting-row low-battery-quality">
        <span class="text">
          <span class="setting-label">{t.settings.lowBatteryQuality}</span>
          <span class="hint">{t.settings.lowBatteryQualityHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={playback?.lowBatteryQuality === true}
          role="switch"
          aria-checked={playback?.lowBatteryQuality === true}
          aria-label={t.settings.lowBatteryQuality}
          disabled={playback === null}
          onclick={toggleLowBatteryQuality}
        >
          <span class="knob"></span>
        </button>
      </label>
      <p class="note">{t.settings.mobileQualityNext}</p>
    </div>
    {/if}
    <div class="group">
      <h2>{t.settings.normalization}</h2>
      <p class="hint">{t.settings.normalizationHint}</p>
      <div class="cache-sizes normalization-modes" role="radiogroup" aria-label={t.settings.normalization}>
        {#each NORMALIZATION_MODES as mode (mode)}
          <button
            class="cache-size hires-mode normalization-mode"
            class:selected={normalization === mode}
            role="radio"
            aria-checked={normalization === mode}
            disabled={normalization === null || exclusive}
            onclick={() => setNormalization(mode)}
          >
            {t.settings.normalizationModes[mode].label}
          </button>
        {/each}
      </div>
      <p class="note">
        {exclusive
          ? t.settings.normalizationExclusive
          : normalization
            ? t.settings.normalizationModes[normalization].note
            : ""}
      </p>
    </div>
    <!-- Égaliseur (#428, Android #467). -->
    {#if playback}
      <div class="group">
        <h2>{t.equalizer.title}</h2>
        <p class="hint">{t.equalizer.hint}</p>
        <EqualizerSettings settings={playback.equalizer} onchange={setEqualizer} />
      </div>
    {/if}
    <div class="group">
      <h2>{t.settings.queueEnd}</h2>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.autoRadio}</span>
          <span class="hint">{t.settings.autoRadioHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={playback?.autoRadio === true}
          role="switch"
          aria-checked={playback?.autoRadio === true}
          aria-label={t.settings.autoRadio}
          disabled={playback === null}
          onclick={toggleAutoRadio}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
    <!-- #368 : filtre de contenu, appliqué côté Rust aux listes chargées ensuite. -->
    <div class="group">
      <h2>{t.settings.content}</h2>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.hideExplicit}</span>
          <span class="hint">{t.settings.hideExplicitHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.content.hideExplicit}
          role="switch"
          aria-checked={ui.settings.content.hideExplicit}
          aria-label={t.settings.hideExplicit}
          onclick={toggleHideExplicit}
        >
          <span class="knob"></span>
        </button>
      </label>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.hideAi}</span>
          <span class="hint">{t.settings.hideAiHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.content.hideAi}
          role="switch"
          aria-checked={ui.settings.content.hideAi}
          aria-label={t.settings.hideAi}
          onclick={toggleHideAi}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
    <!-- #334 : blocages enregistrés chez le service (menu d'un titre pour bloquer). -->
    <div class="group">
      <h2>{t.settings.blocked}</h2>
      <p class="hint">{t.settings.blockedHint}</p>
      <BlockedList />
    </div>
    {#if desktop}
    <div class="group">
      <h2>{t.settings.startup}</h2>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.resumeOnLaunch}</span>
          <span class="hint">{t.settings.resumeOnLaunchHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={playback?.resumeOnLaunch === true}
          role="switch"
          aria-checked={playback?.resumeOnLaunch === true}
          aria-label={t.settings.resumeOnLaunch}
          disabled={playback === null}
          onclick={toggleResumeOnLaunch}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
    {/if}
  {:else}
    {#if desktop}
    <div class="group">
      <h2>{t.settings.window}</h2>
      <p class="hint">{t.settings.closeActionHint}</p>
      <div class="cache-sizes close-actions" role="radiogroup" aria-label={t.settings.closeAction}>
        {#each CLOSE_ACTIONS as action (action)}
          <button
            class="cache-size close-action"
            class:selected={ui.settings.closeAction === action}
            role="radio"
            aria-checked={ui.settings.closeAction === action}
            onclick={() => chooseCloseAction(action)}
          >
            {t.settings.closeActions[action]}
          </button>
        {/each}
      </div>
      <p class="note">{t.settings.closeActionNotes[ui.settings.closeAction]}</p>
    </div>

    <div class="group">
      <h2>{t.settings.startup}</h2>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.autostart}</span>
          <span class="hint">{t.settings.autostartHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={autostart === true}
          role="switch"
          aria-checked={autostart === true}
          aria-label={t.settings.autostart}
          disabled={autostart === null || autostartBusy}
          onclick={toggleAutostart}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
    {/if}

    <!-- #435 : notification des nouvelles sorties des artistes suivis, désactivée par défaut. -->
    <div class="group">
      <h2>{t.settings.notifications}</h2>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.releaseNotifications}</span>
          <span class="hint">{desktop ? t.settings.releaseNotificationsHint : t.settings.releaseNotificationsHintMobile}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.releaseNotifications}
          role="switch"
          aria-checked={ui.settings.releaseNotifications}
          aria-label={t.settings.releaseNotifications}
          onclick={() => setUiToggle("releaseNotifications", !ui.settings.releaseNotifications).catch(fail)}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>

    <!-- #215 : présence Discord, proposée seulement si une application Discord est compilée. -->
    {#if desktop && discordAvailable}
    <div class="group">
      <h2>{t.settings.discord}</h2>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.discordPresence}</span>
          <span class="hint">{t.settings.discordHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.discord}
          role="switch"
          aria-checked={ui.settings.discord}
          aria-label={t.settings.discordPresence}
          onclick={toggleDiscord}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
    {/if}

    <!-- #431 : envoi des écoutes à ListenBrainz, bureau et téléphone. -->
    <div class="group">
      <h2>{t.settings.listenbrainz.title}</h2>
      <p class="hint">{t.settings.listenbrainz.hint}</p>
      <ListenBrainzSettings />
    </div>

    <!-- #413 : raccourcis globaux, actifs hors de la fenêtre (bureau seulement). -->
    {#if desktop && globalShortcutsAvailable}
    <div class="group global-shortcuts-group">
      <h2>{t.settings.globalShortcuts}</h2>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.globalShortcutsEnable}</span>
          <span class="hint">{t.settings.globalShortcutsHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.globalShortcuts.enabled}
          role="switch"
          aria-checked={ui.settings.globalShortcuts.enabled}
          aria-label={t.settings.globalShortcutsEnable}
          onclick={toggleGlobalShortcuts}
        >
          <span class="knob"></span>
        </button>
      </label>
      {#if ui.settings.globalShortcuts.enabled}
        <GlobalShortcutsList />
      {/if}
    </div>
    {/if}

    <!-- #25 : Jam, en ligne par le relais ou sur le réseau local (bureau seulement).
         #452 : un interrupteur la retire de toute l'app. -->
    <div class="group jam">
      <h2>{t.jam.title}</h2>
      <p class="hint">{t.jam.hint}</p>
      <label class="setting-row jam-enable">
        <span class="text">
          <span class="setting-label">{t.jam.enable}</span>
          <span class="hint">{t.jam.enableHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.jam}
          role="switch"
          aria-checked={ui.settings.jam}
          aria-label={t.jam.enable}
          onclick={() => setJamEnabled(!ui.settings.jam).catch(fail)}
        >
          <span class="knob"></span>
        </button>
      </label>
      {#if ui.settings.jam}
        {#if desktop || jam.onlineAvailable}
          <JamPanel />
        {:else}
          <p class="hint">{t.jam.needsRelay}</p>
        {/if}
      {/if}
    </div>

    <!-- #336 : télécommande depuis un téléphone, réseau local, bureau seulement. -->
    {#if desktop}
    <div class="group remote">
      <h2>{t.remote.title}</h2>
      <p class="hint">{t.remote.hint}</p>
      <RemotePanel />
    </div>
    {:else}
    <!-- #414 : ordinateur lié, pour reprendre ou rendre la lecture (Android). -->
    <div class="group handoff">
      <h2>{t.handoff.title}</h2>
      <p class="hint">{t.handoff.hint}</p>
      <LinkedComputerPanel />
    </div>
    {/if}

    <!-- #452 : relais de la Jam en ligne et des transferts, compilé ou choisi. -->
    <div class="group relay">
      <h2>{t.jam.relay.title}</h2>
      <p class="hint">{t.jam.relay.hint}</p>
      <RelaySettings />
    </div>

    <!-- #203 : liens https://tidal.com/… touchés, à donner à l'app sur Android 12+. -->
    {#if links}
    <div class="group">
      <h2>{t.settings.tidalLinks}</h2>
      <div class="setting-row tidal-links">
        <span class="text">
          <span class="setting-label">{t.settings.tidalLinksLabel}</span>
          <span class="hint">{links.enabled ? t.settings.tidalLinksOn : t.settings.tidalLinksOff}</span>
        </span>
        <button class="update-check" onclick={openLinkSettings}>
          {links.enabled ? t.settings.tidalLinksChange : t.settings.tidalLinksEnable}
        </button>
      </div>
    </div>
    {/if}

    <!-- #202 : écoutes gardées localement (accueil, « À la une »). -->
    <div class="group">
      <h2>{t.settings.playHistory}</h2>
      <div class="setting-row">
        <span class="text">
          <span class="hint">{t.settings.playHistoryHint}</span>
        </span>
        <button class="update-check" onclick={clearPlayHistory}>{t.settings.clearPlayHistory}</button>
      </div>
    </div>

    <!-- #192 : journal et rapport de plantage gardés en local, copiés pour une issue. -->
    <div class="group">
      <h2>{t.settings.logs}</h2>
      <p class="hint">{t.settings.logsHint}</p>
      <div class="project-links">
        <button class="update-check" onclick={copyLog}>{t.settings.copyLog}</button>
        <button class="update-check" onclick={copyCrash}>{t.settings.copyCrash}</button>
        {#if desktop}
        <button class="update-check" onclick={() => commands.openLogDir().catch(fail)}>{t.settings.openLogDir}</button>
        {/if}
      </div>
    </div>

    <!-- #330 : bibliothèque exportée en JSON et CSV (bureau : dossier Documents). -->
    {#if desktop}
    <div class="group">
      <h2>{t.settings.libraryExport}</h2>
      <p class="hint">{t.settings.libraryExportHint}</p>
      <div class="project-links">
        {#if libraryExport.running}
        <span class="hint">{t.settings.exportProgress(libraryExport.done, libraryExport.total)}</span>
        <button class="update-check" onclick={cancelLibraryExport}>{t.common.cancel}</button>
        {:else}
        <button class="update-check" onclick={startLibraryExport}>{t.settings.exportLibrary}</button>
        {/if}
      </div>
    </div>
    {/if}

    <!-- #352 : réglages copiés en JSON, collés sur un autre appareil. -->
    <div class="group">
      <h2>{t.settings.backup}</h2>
      <p class="hint">{t.settings.backupHint}</p>
      <div class="project-links">
        <button class="update-check" onclick={copySettings}>{t.settings.copySettings}</button>
        {#if importText === null}
        <button class="update-check" onclick={() => (importText = "")}>{t.settings.importSettings}</button>
        {/if}
      </div>
      {#if importText !== null}
      <textarea class="import-text mono" rows="6" spellcheck="false" placeholder={t.settings.importPlaceholder} bind:value={importText}></textarea>
      <div class="project-links">
        <button class="update-check" onclick={applyImport} disabled={importing || !importText.trim()}>{t.settings.applyImport}</button>
        <button class="update-check" onclick={() => (importText = null)}>{t.common.cancel}</button>
      </div>
      {/if}
    </div>

    <div class="group">
      <h2>{t.settings.updates}</h2>
      <div class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.changelog}</span>
          <span class="hint">{t.settings.changelogHint}</span>
        </span>
        <button class="update-check" onclick={() => (changelogOpen = !changelogOpen)} aria-expanded={changelogOpen}>
          {changelogOpen ? t.settings.hideChangelog : t.settings.showChangelog}
        </button>
      </div>
      {#if changelogOpen}<ChangelogList />{/if}
      <!-- #452 : rien à vérifier sans serveur compilé ; vérification périodique désactivable. -->
      {#if updateServer !== null}
      {#if !desktop || updatesAvailable}
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.autoUpdates}</span>
          <span class="hint">{t.settings.autoUpdatesHint(updateServer)}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.autoUpdates}
          role="switch"
          aria-checked={ui.settings.autoUpdates}
          aria-label={t.settings.autoUpdates}
          onclick={() => setUiToggle("autoUpdates", !ui.settings.autoUpdates).catch(fail)}
        >
          <span class="knob"></span>
        </button>
      </label>
      {/if}
      <div class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.checkUpdates}</span>
          <span class="hint">
            {#if desktop && !updatesAvailable}
              {t.settings.updatesManual}
            {:else if updateCheck === "uptodate"}
              {t.settings.upToDate}
            {:else if updateCheck === "downloading"}
              {t.settings.updateDownloading(downloadingVersion)}
            {:else if updateCheck === "ready"}
              {desktop ? t.settings.updateReady : t.settings.updateAvailable}
            {:else}
              {ui.settings.autoUpdates ? t.settings.updateAuto : t.settings.updateManualOnly}
            {/if}
          </span>
        </span>
        {#if !desktop || updatesAvailable}
        <button class="update-check" onclick={checkUpdates} disabled={updateCheck === "checking" || updateCheck === "downloading"}>
          {updateCheck === "checking" ? t.settings.checking : t.settings.check}
        </button>
        {/if}
      </div>
      {/if}
    </div>
    <div class="group about">
      <h2>{t.settings.about}</h2>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <p><span class="app-name">Seaglass</span> <span class="mono" onclick={clickVersion}>v{__APP_VERSION__}</span></p>
      {#if creditsOpen}<CreditsRoll onClose={() => (creditsOpen = false)} />{/if}
      <p><strong>{t.settings.disclaimer}</strong></p>
      <p>{t.settings.streamingOnly}</p>
      <p>{t.settings.license}</p>
      <!-- #188 : URLs fixes côté Rust, ouvertes dans le navigateur du système. -->
      <div class="project-links">
        <button class="update-check" onclick={() => openProject("repo")}>{t.settings.sourceCode}</button>
        <button class="update-check" onclick={() => openProject("newIssue")} title={t.settings.reportIssueHint}>
          {t.settings.reportIssue}
        </button>
      </div>
    </div>

    <!-- #189 : sous À propos, sur les deux plateformes (plus dans l'en-tête du bureau). -->
    {#if onLogout}
    <div class="group">
      <h2>{t.settings.account}</h2>
      <button class="update-check logout" onclick={onLogout}>{t.common.logout}</button>
    </div>
    {/if}
  {/if}
</section>

<style>
  .settings {
    padding: 24px;
    max-width: 1040px;
  }

  h1 {
    margin: 0 10px;
    font-size: 26px;
  }

  .tabs {
    display: flex;
    gap: 6px;
    margin: 16px 10px 20px;
  }

  .tabs button {
    padding: 6px 14px;
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--bg-raised);
  }

  .tabs button.active {
    color: var(--bg);
    background: var(--text);
  }

  .group {
    margin: 0 10px 28px;
  }

  h2 {
    margin: 0 0 4px;
    font-size: 17px;
  }

  .hint {
    margin: 0 0 14px;
    max-width: 640px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .note {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  /* Qualité par réseau (#365, Android) : sous-titres de la même section. */
  .network-quality {
    margin: 18px 0 10px;
    font-size: 14px;
    font-weight: 600;
  }

  .network-quality:first-of-type {
    margin-top: 0;
  }

  .low-battery-quality {
    margin-top: 20px;
  }

  .cache-sizes {
    display: inline-flex;
    gap: 4px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
  }

  .cache-size {
    min-width: 76px;
    padding: 6px 14px;
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

  /* Mêmes sélecteurs que le cache (styles des thèmes compris), en texte courant. */
  .hires-mode {
    min-width: 0;
    font-family: inherit;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    max-width: 640px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .setting-row .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .setting-row .setting-label {
    font-weight: 500;
  }

  .setting-row .hint {
    margin: 0;
    font-size: 12px;
  }

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
    opacity: 0.5;
    cursor: default;
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

  .update-check:disabled {
    opacity: 0.6;
    cursor: default;
  }

  /* #189 : action destructrice en rouge, au rouge de chaque thème (`--danger`). `:root` passe
     devant les surcharges de `.update-check` des fichiers de thème ; le fond reste le leur. */
  :global(:root) .update-check.logout {
    color: var(--danger);
    border-color: var(--danger) !important;
  }

  :global(:root) .update-check.logout:hover {
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }

  .about p {
    margin: 6px 0;
    color: var(--text-muted);
  }

  .about strong,
  .app-name {
    font-weight: 600;
    color: var(--text);
  }

  .import-text {
    display: block;
    width: 100%;
    max-width: 640px;
    margin: 12px 0;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text);
    background: var(--bg-raised);
    font-size: 12px;
    resize: vertical;
  }

  .project-links {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 14px;
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  /* Studio : angles vifs. Atelier : angles à peine arrondis, pas de fond. */
  :global([data-theme="studio"]) .tabs button,
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .setting-switch,
  :global([data-theme="studio"]) .setting-switch .knob,
  :global([data-theme="studio"]) .update-check {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .setting-row {
    border-radius: 2px;
  }

  :global([data-theme="atelier"]) .setting-row {
    background: transparent;
  }
</style>
