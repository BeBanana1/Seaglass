// Jam, relay, remote control and handoff in the mock backend.

import { params, type Handlers } from "./core";
import { manual } from "./player";
import type { RelayView } from "../ipc";

// Jam (#25): `?jam=1` shows it started online, `?jam=local` on the local network,
// `?jam=reconnecting` reconnecting; with `manual=1`, the first track of the manual queue is
// marked as added by a guest. Two guests (one of them banned) in the host's controls (#288).
const fakeQr = (() => {
  let cells = "";
  for (let y = 0; y < 21; y++)
    for (let x = 0; x < 21; x++)
      if ((x * 7 + y * 13 + x * y) % 3 === 0) cells += `M${x + 2} ${y + 2}h1v1h-1z`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 25 25" shape-rendering="crispEdges"><rect width="25" height="25" fill="#ffffff"/><path fill="#000000" d="${cells}"/></svg>`;
})();
function mockJam(mode: "online" | "local", reconnecting = false) {
  const first = manual[0];
  return {
    mode,
    url:
      mode === "online"
        ? "https://jam.bebanana.ch/j/3f9c2a7e5d1b4c08a6e2f1d9b7c3a5e0/"
        : "http://192.168.1.20:47800/",
    code: "K7QX2M",
    lan: true,
    qrSvg: fakeQr,
    connection: reconnecting ? ("reconnecting" as const) : ("online" as const),
    reason: reconnecting ? "relais injoignable" : null,
    guests: first ? { [first.id]: "Léa" } : {},
    rules: {
      guestControls: true,
      maxPending: null as number | null,
      albums: true,
      guestVolume: false,
      guestVolumeMax: 80,
      skipVote: false,
      skipVotes: 3,
    },
    addsOpen: true,
    members: [
      { id: 1, name: "Léa" as string | null, added: 4, banned: false },
      { id: 2, name: null as string | null, added: 31, banned: true },
    ],
  };
}
// `?relay=off`: relay turned off; `?relay=none`: app built without a relay (#452).
const relayParam = params.get("relay");
let relay: RelayView = {
  enabled: relayParam !== "off",
  url: null,
  defaultUrl: relayParam === "none" ? null : "https://jam.example.org",
  customKey: false,
  usable: relayParam !== "off" && relayParam !== "none",
};
const jamParam = params.get("jam");
let jam: ReturnType<typeof mockJam> | null =
  jamParam === "1" || jamParam === "reconnecting"
    ? mockJam("online", jamParam === "reconnecting")
    : jamParam === "local"
      ? mockJam("local")
      : null;
/** Remote control (#336): `?remote=1` shows it on; `?handoff=1` accepts handoffs (#414).
 *  Phone: `?platform=android&linked=1` has a linked computer. */
let remoteCode = "H4KX9QMT2WZP";
let remoteOn = params.get("remote") === "1";
let handoffOn = params.get("handoff") === "1";
let pairingUntil: number | null = null;
const mockRemote = () => ({
  remote: remoteOn,
  handoff: handoffOn,
  url: "http://192.168.1.20:47801/",
  code: remoteCode,
  lan: true,
  qrSvg: fakeQr,
  pairingUntil,
  relay: relayParam !== "off" && relayParam !== "none",
});
let remote: ReturnType<typeof mockRemote> | null = remoteOn || handoffOn ? mockRemote() : null;
const remoteUpdate = () => (remote = remoteOn || handoffOn ? mockRemote() : null);
let linkedComputer: { host: string; syncedAt: number | null; online: boolean } | null =
  params.get("linked") === "1"
    ? { host: "192.168.1.20:47801", syncedAt: Date.now() - 3_600_000, online: true }
    : null;

export const jamHandlers: Handlers = {
  jam_status: () => jam,
  jam_online_available: () => relay.usable,
  relay_settings: () => relay,
  // An address containing "down" imitates an unreachable relay.
  set_relay_settings: ({ change }) => {
    const c = change as { enabled: boolean; url: string | null; key?: string | null };
    const url = c.url?.trim().replace(/\/+$/, "") || null;
    if (c.enabled && url && !url.startsWith("https://"))
      throw "adresse du relais invalide : https://… attendu";
    if (c.enabled && url?.includes("down")) throw "relais injoignable : vérifiez l'adresse";
    const customKey = c.key === undefined || c.key === null ? relay.customKey : c.key.trim() !== "";
    relay = {
      ...relay,
      enabled: c.enabled,
      url: url === relay.defaultUrl ? null : url,
      customKey,
      usable: c.enabled && !!(url ?? relay.defaultUrl),
    };
    return relay;
  },
  start_jam: (args) => (jam ??= mockJam(args.mode === "local" ? "local" : "online")),
  stop_jam: () => void (jam = null),
  remote_status: () => remote,
  start_remote: () => ((remoteOn = true), remoteUpdate()),
  stop_remote: () => ((remoteOn = false), remoteUpdate()),
  set_handoff_accepted: ({ enabled }) => ((handoffOn = !!enabled), remoteUpdate()),
  linked_computer: () => linkedComputer,
  link_computer: ({ link }) => {
    const m = /^\s*(?:http:\/\/)?([^/#\s]+)\/?[#\s]/.exec(String(link));
    if (!m)
      throw new Error(
        "adresse non reconnue : copiez-la depuis Réglages › Application › Télécommande sur l'ordinateur",
      );
    return (linkedComputer = { host: m[1], syncedAt: null, online: true });
  },
  unlink_computer: () => void (linkedComputer = null),
  handoff_pull: () => undefined,
  handoff_push: () => undefined,
  sync_history: () => (linkedComputer ? (linkedComputer.syncedAt = Date.now()) : null),
  open_handoff_pairing: () => {
    if (!handoffOn) throw new Error("activez d'abord « Accepter les transferts »");
    pairingUntil = Date.now() + 10 * 60_000;
    return (remote = mockRemote());
  },
  renew_remote_code: () => {
    remoteCode = "QZ7M3KD8XF2N";
    return remote ? (remote = mockRemote()) : null;
  },
  set_jam_rules: (args) =>
    jam ? (jam = { ...jam, rules: args.rules as NonNullable<typeof jam>["rules"] }) : null,
  set_jam_adds_open: (args) => (jam ? (jam = { ...jam, addsOpen: args.open === true }) : null),
  set_jam_guest_banned: (args) =>
    jam
      ? (jam = {
          ...jam,
          members: jam.members.map((m) =>
            m.id === args.id ? { ...m, banned: args.banned === true } : m,
          ),
        })
      : null,
};
