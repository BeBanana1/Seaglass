// --- Connection -----------------------------------------------------------------

function showJoin(error) {
  closeEvents();
  $("remote").hidden = true;
  $("join").hidden = false;
  const err = $("join-error");
  err.hidden = !error;
  err.textContent = error || "";
  $("code").value = code;
  $("code").focus();
}

async function join(candidate) {
  code = candidate.replace(/[\s-]/g, "").toUpperCase();
  try {
    await api("join");
  } catch (e) {
    if (!lostAccess(e)) showJoin(message(e.kind));
    return;
  }
  localStorage.setItem(CODE_KEY, code);
  $("join").hidden = true;
  $("remote").hidden = false;
  openEvents();
}

function openEvents() {
  closeEvents();
  events = new EventSource(`api/events?code=${encodeURIComponent(code)}`);
  events.onmessage = (event) => {
    try {
      render(JSON.parse(event.data));
    } catch {
      // Unreadable message: the next one will replace it.
    }
  };
  // The browser reconnects on its own; a code changed from the computer makes it fail in a
  // loop: check it once.
  events.onerror = async () => {
    try {
      await api("join");
    } catch (e) {
      lostAccess(e);
    }
  };
}

function closeEvents() {
  events?.close();
  events = null;
}

// --- Player -------------------------------------------------------------------

function position() {
  if (!state) return 0;
  const elapsed = state.paused || state.loading ? 0 : (performance.now() - receivedAt) / 1000;
  return Math.min(state.position + elapsed, state.duration || Infinity);
}

function tick() {
  if (!state || dragging.position) return;
  const pos = position();
  $("position").value = String(Math.floor(pos));
  $("elapsed").textContent = clock(pos);
}

function render(next) {
  state = next;
  receivedAt = performance.now();
  $("player").classList.toggle("loading", next.loading);

  const now = $("now");
  now.replaceChildren();
  if (next.now) {
    now.append(cover(next.now.cover));
    const meta = el("div", "meta");
    meta.append(
      el("div", "title", next.now.title),
      el("div", "sub", next.now.artists),
      el("div", "album", next.now.album),
    );
    now.append(meta);
  } else {
    now.append(el("div", "muted", t.nothing));
  }

  const seek = $("position");
  seek.max = String(next.duration || 0);
  seek.disabled = !next.now || !next.duration;
  $("total").textContent = clock(next.duration);
  tick();

  const play = $("play");
  play.textContent = next.paused ? "▶" : "❚❚";
  play.title = next.paused ? t.play : t.pause;
  play.setAttribute("aria-label", play.title);
  play.disabled = !next.now;
  $("previous").disabled = !next.now;
  $("next").disabled = !next.hasNext;

  const shuffle = $("shuffle");
  shuffle.classList.toggle("on", next.shuffle);
  shuffle.setAttribute("aria-pressed", String(next.shuffle));
  const repeat = $("repeat");
  repeat.classList.toggle("on", next.repeat !== "off");
  repeat.classList.toggle("one", next.repeat === "one");
  repeat.setAttribute("aria-pressed", String(next.repeat !== "off"));

  const mute = $("mute");
  mute.classList.toggle("silent", next.muted || next.volume === 0);
  mute.classList.toggle("on", next.muted);
  mute.setAttribute("aria-pressed", String(next.muted));
  if (!dragging.volume) {
    $("volume").value = String(next.volume);
    $("volume-value").textContent = String(next.volume);
  }

  renderQueue(next);
}

function renderQueue(next) {
  const manual = $("manual");
  manual.replaceChildren(
    ...next.manual.map((track, index) =>
      row(track, track.artists, () => send("jump", { part: "manual", index, trackId: track.id }), [
        smallButton(t.remove, "✕", () => send("remove", { trackId: track.id })),
      ]),
    ),
  );
  const upcoming = $("upcoming");
  upcoming.replaceChildren(
    ...next.upcoming.map((track, index) =>
      row(track, track.artists, () =>
        send("jump", { part: "upcoming", index, trackId: track.id }),
      ),
    ),
  );
  $("manual-title").hidden = !next.manual.length;
  $("upcoming-title").hidden = !next.upcoming.length;
  $("queue-empty").hidden = Boolean(next.manual.length || next.upcoming.length);
}

function setupPlayer() {
  $("play").addEventListener("click", () => {
    if (!state) return;
    // Explicit, not a toggle: a double tap does not trigger the pause again.
    state.paused = !state.paused;
    send("pause", { paused: state.paused });
    render(state);
  });
  for (const [id, label] of [
    ["previous", t.previous],
    ["next", t.next],
    ["shuffle", t.shuffle],
    ["repeat", t.repeat],
    ["mute", t.mute],
  ]) {
    $(id).title = label;
    $(id).setAttribute("aria-label", label);
  }
  $("previous").addEventListener("click", () => send("previous"));
  $("next").addEventListener("click", () => send("next"));
  $("shuffle").addEventListener("click", () => state && send("shuffle", { enabled: !state.shuffle }));
  $("repeat").addEventListener("click", () => {
    if (!state) return;
    const mode = { off: "all", all: "one", one: "off" }[state.repeat] || "off";
    send("repeat", { mode });
  });
  $("mute").addEventListener("click", () => state && send("mute", { muted: !state.muted }));

  // Sliders: one command on release, not during the gesture.
  const seek = $("position");
  seek.addEventListener("input", () => {
    dragging.position = true;
    $("elapsed").textContent = clock(Number(seek.value));
  });
  seek.addEventListener("change", async () => {
    const seconds = Number(seek.value);
    if (state) {
      state.position = seconds;
      receivedAt = performance.now();
    }
    dragging.position = false;
    await send("seek", { seconds });
  });
  const volume = $("volume");
  volume.addEventListener("input", () => {
    dragging.volume = true;
    $("volume-value").textContent = volume.value;
  });
  volume.addEventListener("change", async () => {
    dragging.volume = false;
    await send("volume", { level: Number(volume.value) });
  });
  setInterval(tick, 250);
}
