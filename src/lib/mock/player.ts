// Player in the mock backend: playback state, queue, playback settings and output device.

import { blindTestMock } from "./blindTest";
import { allTracks, emit, fx, params, radioPage, type Handlers } from "./core";
import type {
  AudioDevices,
  EqFilter,
  EqFilterKind,
  EqProfile,
  OutputDevice,
  PlaySource,
  PlaybackSettings,
  PlayerState,
  QueueView,
  SleepRequest,
  SleepState,
  StreamFormat,
  Track,
} from "../ipc";

let context: Track[] = fx.album.tracks;
let index = Math.min(1, context.length - 1);
/** `?autoradio=1`: the rest of the queue is the current track's radio (#78). */
let radioSeed: Track | null = null;
if (params.has("autoradio") && context[index]) {
  radioSeed = context[index];
  context = [radioSeed, ...radioPage.tracks.filter((t) => t.id !== radioSeed?.id)];
  index = 0;
}
/** "Playing from" (#124): the demo album, except on the seed track of a radio. */
let playSource: PlaySource | null = radioSeed
  ? null
  : { kind: "album", id: fx.album.album.id, title: fx.album.album.title };
/** `?manual=1`: a few tracks in the manual queue, to reorder them (#80). */
export const manual: Track[] = params.has("manual") ? fx.playlist.tracks.slice(0, 4) : [];
/** The queue before the last clear or removal (#349), like `Player::undo_queue`. */
let queueUndo: {
  manual: Track[];
  context: Track[];
  index: number;
  radioSeed: Track | null;
  playSource: PlaySource | null;
} | null = null;
/** `?volume=60` or `?volume=muted`: audio chain no longer bit-perfect. */
const volumeParam = params.get("volume");
/** `?hiresedition=Thriller`: stream from another hi-res edition (#18), whose album can be opened (#111). */
const hiresEditionParam = params.get("hiresedition");
export const hiresEdition =
  hiresEditionParam === null ? null : { id: fx.album.album.id, title: hiresEditionParam };
/** `?hiresoffer=Thriller`: "Ask" setting, banner offering a hi-res edition (#59). */
const hiresOfferParam = params.get("hiresoffer");

/** In-memory cache (#453): `?cached=whole` (whole track), `?cached=40` (40 s ahead), 90 s
 *  ahead by default. */
function mockCached(position: number, duration: number): number {
  const param = params.get("cached");
  const ahead = param === "whole" ? Infinity : Number(param ?? 90) || 0;
  return Math.min(position + ahead, duration);
}

/** `?sleep=30` (30 min timer), `?sleep=track` (end of the track, #85) or `?sleep=context` (end of the queue, #436). */
function mockSleep(param: string | null): SleepState | null {
  if (param === "track") return { kind: "endOfTrack" };
  if (param === "context") return { kind: "endOfContext" };
  const minutes = Number(param);
  return param && minutes > 0 ? { kind: "minutes", endsAt: Date.now() + minutes * 60_000 } : null;
}

function offerFormat(album: string): StreamFormat {
  const hiresEdition = { id: fx.album.album.id, title: album };
  return {
    quality: "HI_RES_LOSSLESS",
    codec: "flac",
    bitDepth: 24,
    sampleRate: 192000,
    bitrate: 5071,
    hiresEdition,
  };
}

export const state: PlayerState = {
  // `?idle=1`: nothing playing (empty "Now playing" view, #399).
  track: params.has("idle") ? null : (context[index] ?? null),
  stream: {
    quality: "HI_RES_LOSSLESS",
    codec: "flac",
    bitDepth: 24,
    sampleRate: 44100,
    bitrate: 1854,
    hiresEdition,
  },
  output: { format: "s32", samplerate: 44100, channelCount: 2 },
  // Computed below, from the mock exclusive mode.
  deviceFormat: null,
  outputDriver: "wasapi",
  exclusive: false,
  // Computed below, after the mock playback settings.
  outputDevice: null,
  loading: false,
  paused: params.has("paused"),
  buffering: false,
  idle: params.has("idle"),
  timePos: 48,
  duration: context[index]?.duration ?? 0,
  cached: mockCached(48, context[index]?.duration ?? 0),
  nextReady: params.has("nextready"),
  shuffle: false,
  repeat: "off",
  hasPrevious: true,
  hasNext: true,
  volume: {
    level:
      volumeParam && volumeParam !== "muted"
        ? Math.min(100, Math.max(0, Number(volumeParam) || 0))
        : 100,
    muted: volumeParam === "muted",
  },
  hiresOffer: hiresOfferParam ? offerFormat(hiresOfferParam) : null,
  // Computed below, after the mock playback settings.
  normalization: null,
  equalizer: null,
  sleep: mockSleep(params.get("sleep")),
  // `?ended=1`: queue ended ("The End" card of the Séance theme, #399). Soundtrack: true for
  // the Interstellar album of the fixtures, like the backend's list.
  soundtrack: context[index]?.album.id === 156749593,
  ended: params.has("ended"),
  blindTest: false,
  /** `?mpv=cpu`, `?mpv=stopped` or `?mpv=missing`: "mpv is not running" banner (#92, #261). */
  mpvIssue:
    ({ cpu: "cpuUnsupported", stopped: "stopped", missing: "missing" } as const)[
      params.get("mpv") ?? ""
    ] ?? null,
};

// `?normalization=track|album`: normalization on (mock gain of −6.5 dB).
const normalizationParam = params.get("normalization") as PlaybackSettings["normalization"] | null;
// `?device=missing`: the chosen device is unplugged; `?device=dac`: it is plugged in.
// Windows names its outputs in the system language: English with `?lang=en` (screenshots).
const speakersLabel = params.get("lang") === "en" ? "Speakers" : "Haut-parleurs";
const DAC = {
  id: "wasapi/{27264a86-c734-4c85-876c-dcfeb567529f}",
  name: `${speakersLabel} (Focusrite USB Audio)`,
};
const SPEAKERS = {
  id: "wasapi/{0d7e4c2b-58a1-4c5e-9f1e-3b2a9c1d7e44}",
  name: `${speakersLabel} (Realtek(R) Audio)`,
};
const deviceParam = params.get("device");
/** Demo profile (taken from AutoEQ, Sennheiser HD 600). */
const DEMO_EQ: EqProfile = {
  id: 1,
  name: "Sennheiser HD 600",
  preamp: -6.2,
  filters: [
    { kind: "lowShelf", freq: 105, gain: 5.5, q: 0.7 },
    { kind: "peaking", freq: 2386, gain: -2.1, q: 1.26 },
    { kind: "peaking", freq: 4483, gain: 3.3, q: 3.31 },
    { kind: "peaking", freq: 159, gain: -1, q: 0.44 },
    { kind: "highShelf", freq: 10000, gain: -1.4, q: 0.7 },
  ],
};
export let playback: PlaybackSettings = {
  cacheSize: 64,
  hiresUpgrade: hiresOfferParam ? "ask" : "auto",
  audioDevice: deviceParam ? DAC : null,
  autoRadio: radioSeed !== null,
  pauseOnUnplug: true,
  resumeOnLaunch: false,
  deviceProfiles: params.has("profiles"),
  wifiQuality: "lossless",
  cellularQuality: "lossless",
  lowBatteryQuality: false,
  normalization: normalizationParam ?? "off",
  equalizer: { enabled: params.has("eq"), active: 1, profiles: [DEMO_EQ] },
  exclusive: params.has("exclusive"),
};
const audioDevices: AudioDevices = {
  devices: deviceParam === "missing" ? [SPEAKERS] : [SPEAKERS, DAC],
  systemDefault: SPEAKERS.id,
};

/** Same logic as `player/output.rs`: the chosen device if it is plugged in, otherwise the system default. */
function outputDevice(): OutputDevice {
  const chosen = playback.audioDevice;
  const present = chosen && audioDevices.devices.some((d) => d.id === chosen.id);
  if (chosen && present) return { name: chosen.name, systemDefault: false, missing: null };
  return { name: SPEAKERS.name, systemDefault: true, missing: chosen ? chosen.name : null };
}
state.outputDevice = outputDevice();
/** Mock gain: −6.5 dB per track, −4 dB per album. */
function mockGain(): number | null {
  return playback.normalization === "off" ? null : playback.normalization === "track" ? -6.5 : -4;
}
state.normalization = mockGain();
/** Like `EqSettings::applied`: name of the applied profile. */
function mockEqualizer(): string | null {
  const eq = playback.equalizer;
  return (eq.enabled && eq.profiles.find((p) => p.id === eq.active)?.name) || null;
}
state.equalizer = mockEqualizer();
/** Like `player/exclusive.rs` and mpv: in exclusive mode the stream's format goes out as is and
 *  normalization is suspended; in shared mode, Windows imposes its mix format. */
function applyExclusive() {
  state.exclusive = playback.exclusive;
  if (playback.exclusive) state.normalization = null;
  state.deviceFormat = !state.output
    ? null
    : playback.exclusive
      ? { ...state.output }
      : { format: "float", samplerate: 48000, channelCount: state.output.channelCount };
}
applyExclusive();

export const blind = blindTestMock({ state, track: (id) => allTracks.get(id), emit });

function queueView(): QueueView {
  return {
    manual: [...manual],
    upcoming: context.slice(index + 1, index + 51),
    radio: radioSeed,
    source: playSource,
  };
}

function setCurrent(track: Track | undefined) {
  if (!track) return;
  Object.assign(state, {
    track,
    timePos: 0,
    duration: track.duration,
    cached: mockCached(0, track.duration),
    nextReady: false,
    paused: false,
    idle: false,
    soundtrack: track.album.id === 156749593,
    ended: false,
    hiresOffer:
      !track.hires && hiresOfferParam !== null && playback.hiresUpgrade === "ask"
        ? offerFormat(hiresOfferParam)
        : null,
    // With `?hiresedition=…` and the setting on, a track that is not hi-res is "replaced" by
    // another edition, as the backend would do.
    ...(() => {
      const upgraded = !track.hires && hiresEdition !== null && playback.hiresUpgrade === "auto";
      const hires = track.hires || upgraded;
      return {
        stream: hires
          ? {
              quality: "HI_RES_LOSSLESS",
              codec: "flac",
              bitDepth: 24,
              sampleRate: 48000,
              bitrate: 2016,
              hiresEdition: upgraded ? hiresEdition : null,
            }
          : {
              quality: "LOSSLESS",
              codec: "flac",
              bitDepth: 16,
              sampleRate: 44100,
              bitrate: 893,
              hiresEdition: null,
            },
        output: {
          format: hires ? "s32" : "s16",
          samplerate: hires ? 48000 : 44100,
          channelCount: 2,
        },
      };
    })(),
  });
  state.hasNext = manual.length > 0 || index + 1 < context.length;
  applyExclusive();
  emit("player-state", state);
  emit("queue-state", queueView());
}

/** 250 ms of playback: `?ticks=1`, or called directly by a measurement (the timers of a hidden
 *  page are slowed down). */
export function playbackTick() {
  if (state.paused || state.idle || state.loading || !(state.track || state.blindTest)) return;
  state.timePos = Math.min(state.timePos + 0.25, state.duration);
  blind.tick();
  // Website demo (#28): at the end of a track, the queue moves on as with the real player.
  if (__DEMO__ && !state.blindTest && state.timePos >= state.duration) return demoTrackEnd();
  emit("player-state", state);
}
if (__DEMO__ || params.has("ticks")) setInterval(playbackTick, 250);

/** End of a track in the demo: Repeat One starts it again, then the manual queue, the
 *  context (from the start again with Repeat All), or the end of the queue. */
function demoTrackEnd() {
  if (state.repeat === "one") {
    state.timePos = 0;
    emit("player-state", state);
  } else if (manual.length) setCurrent(manual.shift());
  else if (index + 1 < context.length) setCurrent(context[++index]);
  else if (state.repeat === "all") setCurrent(context[(index = 0)]);
  else {
    state.paused = true;
    state.timePos = 0;
    emit("player-state", state);
  }
}

export const playerHandlers: Handlers = {
  player_state: () => state,
  queue_state: () => queueView(),
  queue_track_ids: () => {
    const v = queueView();
    return [...v.manual, ...v.upcoming].map((t) => t.id);
  },
  playback_settings: () => playback,
  // Preview (#492): battery saver on, as on the S21.
  quality_cap: () =>
    playback.lowBatteryQuality && playback.wifiQuality !== "low"
      ? { quality: "low", chosen: playback.wifiQuality, reason: "powerSave", battery: 61 }
      : null,
  // Same validation as the backend: only the offered sizes are accepted.
  set_playback_settings: ({ settings }) => {
    const next = settings as PlaybackSettings;
    if (![32, 64, 128, 256].includes(next.cacheSize)) {
      throw new Error(`taille de cache non prise en charge : ${next.cacheSize} Mio`);
    }
    playback = JSON.parse(JSON.stringify(next));
    state.outputDevice = outputDevice();
    state.normalization = mockGain();
    state.equalizer = mockEqualizer();
    applyExclusive();
    emit("player-state", state);
  },
  // Simplified version of `player/autoeq.rs`: enough to try the import.
  parse_autoeq: ({ text }) => {
    const kinds: Record<string, EqFilterKind> = {
      PK: "peaking",
      PEQ: "peaking",
      LSC: "lowShelf",
      LS: "lowShelf",
      HSC: "highShelf",
      HS: "highShelf",
    };
    const num = (line: string, key: string) =>
      Number(new RegExp(`${key}\\s+(-?[\\d.]+)`, "i").exec(line)?.[1]);
    const filters: EqFilter[] = [];
    for (const line of String(text).split(/\r?\n/)) {
      const m = /^Filter\s*\d*:\s*ON\s+(\w+)/i.exec(line.trim());
      const kind = m && kinds[m[1].toUpperCase()];
      if (kind)
        filters.push({
          kind,
          freq: num(line, "Fc"),
          gain: num(line, "Gain"),
          q: num(line, "Q") || 0.71,
        });
    }
    if (!filters.length) throw new Error("ce texte n'est pas un profil AutoEQ (ParametricEQ.txt)");
    const preamp = /Preamp:\s*(-?[\d.]+)/i.exec(String(text));
    return {
      id: 0,
      name: "",
      preamp: preamp ? Number(preamp[1]) : null,
      filters,
    } satisfies EqProfile;
  },
  audio_devices: () => audioDevices,
  play_tracks: ({ trackIds, startIndex, shuffle, source }) => {
    context = (trackIds as number[]).map((id) => allTracks.get(id)).filter((t): t is Track => !!t);
    radioSeed = null;
    playSource = source ? (JSON.parse(JSON.stringify(source)) as PlaySource) : null;
    index = shuffle ? Math.floor(Math.random() * context.length) : Number(startIndex);
    // `null` keeps the mode, `false` ("Play") turns it off (#526).
    if (typeof shuffle === "boolean") state.shuffle = shuffle;
    setCurrent(context[index]);
  },
  play_next: ({ trackId }) => {
    const t = allTracks.get(Number(trackId));
    if (t) manual.unshift(t);
    emit("queue-state", queueView());
  },
  add_to_queue: ({ trackId }) => {
    const t = allTracks.get(Number(trackId));
    if (t) manual.push(t);
    emit("queue-state", queueView());
  },
  enqueue_tracks: ({ trackIds, next }) => {
    const ts = (trackIds as number[]).map((id) => allTracks.get(id)).filter((t): t is Track => !!t);
    if (next) manual.unshift(...ts);
    else manual.push(...ts);
    emit("queue-state", queueView());
  },
  remove_from_queue: ({ index: i }) => {
    queueUndo = { manual: [...manual], context, index, radioSeed, playSource };
    manual.splice(Number(i), 1);
    emit("queue-state", queueView());
  },
  move_in_queue: ({ from, to }) => {
    const [f, t] = [Number(from), Number(to)];
    if (f === t || f >= manual.length || t >= manual.length) return;
    manual.splice(t, 0, ...manual.splice(f, 1));
    emit("queue-state", queueView());
  },
  answer_hires_offer: ({ accept }) => {
    if (!state.hiresOffer) return;
    if (accept) state.stream = state.hiresOffer;
    state.hiresOffer = null;
    emit("player-state", state);
  },
  set_sleep_timer: ({ timer }) => {
    const req = timer as SleepRequest | null;
    if (req?.kind === "minutes" && (req.minutes < 1 || req.minutes > 600)) {
      throw new Error(`durée du minuteur invalide : ${req.minutes} min (1 à 600)`);
    }
    state.sleep = !req
      ? null
      : req.kind === "minutes"
        ? { kind: "minutes", endsAt: Date.now() + req.minutes * 60_000 }
        : { kind: req.kind };
    emit("player-state", state);
  },
  restore_queue: () => {
    if (!queueUndo) return false;
    manual.splice(0, manual.length, ...queueUndo.manual);
    ({ context, index, radioSeed, playSource } = queueUndo);
    queueUndo = null;
    state.hasNext = true;
    emit("player-state", state);
    emit("queue-state", queueView());
    return true;
  },
  clear_queue: () => {
    queueUndo = { manual: [...manual], context, index, radioSeed, playSource };
    manual.length = 0;
    radioSeed = null;
    playSource = null;
    context = context.slice(index, index + 1);
    index = 0;
    state.hasNext = false;
    emit("player-state", state);
    emit("queue-state", queueView());
  },
  play_from_queue: ({ part, index: i, trackId }) => {
    const at = Number(i);
    if (part === "manual") {
      if (manual[at]?.id !== Number(trackId)) return;
      manual.splice(0, at);
      return setCurrent(manual.shift());
    }
    if (context[index + 1 + at]?.id !== Number(trackId)) return;
    index += 1 + at;
    setCurrent(context[index]);
  },
  next: () => {
    if (manual.length) return setCurrent(manual.shift());
    if (index + 1 < context.length) setCurrent(context[++index]);
  },
  previous: () => {
    if (state.timePos > 3 || index === 0) {
      state.timePos = 0;
      emit("player-state", state);
    } else setCurrent(context[--index]);
  },
  toggle_pause: () => {
    state.paused = !state.paused;
    emit("player-state", state);
  },
  seek: ({ seconds }) => {
    state.timePos = Number(seconds);
    emit("player-state", state);
  },
  set_shuffle: ({ enabled }) => {
    state.shuffle = Boolean(enabled);
    emit("player-state", state);
  },
  set_repeat: ({ mode }) => {
    state.repeat = mode as PlayerState["repeat"];
    emit("player-state", state);
  },
  // Same rules as `player::volume::Volume` on the Rust side.
  set_volume: ({ volume }) => {
    const level = Number(volume);
    if (!Number.isInteger(level) || level < 0 || level > 100) throw new Error("volume invalide");
    state.volume.level = level;
    if (level > 0) state.volume.muted = false;
    emit("player-state", state);
  },
  set_muted: ({ muted }) => {
    state.volume.muted = Boolean(muted);
    if (!state.volume.muted && state.volume.level === 0) state.volume.level = 50;
    emit("player-state", state);
  },
};
