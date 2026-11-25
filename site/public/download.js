// Script of the download page (site/src/components/DownloadCards.astro, #517), served from
// public/: the server's CSP forbids inline scripts. Each platform is published on its own by
// its release script: its card only appears if its manifest is online and points to an HTTPS address.
const platforms = ["windows", "android", "linux"];

// The visitor's system (#48), whose card goes first. Android before Linux: its user agent says
// "Linux" too. ChromeOS, macOS and iOS get no card of their own.
function detect() {
  const ua = navigator.userAgent;
  if (/Android/i.test(ua)) return "android";
  if (/Windows/i.test(ua)) return "windows";
  if (/CrOS|iPhone|iPad|Macintosh/i.test(ua)) return null;
  if (/Linux|X11/i.test(ua)) return "linux";
  return null;
}

function show(id, m) {
  document.getElementById(`${id}-link`).href = m.url;
  const size = (m.size / 1048576).toLocaleString("en", { maximumFractionDigits: 1 });
  document.getElementById(`${id}-meta`).textContent = `Version ${m.version} · ${size} MB`;
  document.getElementById(id).hidden = false;
}

function recommend(id) {
  const card = document.getElementById(id);
  card.classList.add("sg-recommended");
  card.querySelector(".sg-badge").hidden = false;
  document.getElementById("downloads").prepend(card);
}

// "What's new": notes of the Windows manifest (one "- …" line per change).
function showNotes(m) {
  if (typeof m.notes !== "string") return;
  const list = document.getElementById("notes");
  for (const line of m.notes.split(/\r?\n/)) {
    const text = line.replace(/^\s*-\s*/, "").trim();
    if (!text) continue;
    const item = document.createElement("li");
    item.textContent = text;
    list.append(item);
  }
  if (!list.childElementCount) return;
  document.getElementById("notes-title").textContent = `What's new in ${m.version}`;
  document.getElementById("notes-card").hidden = false;
}

Promise.all(
  platforms.map((id) =>
    fetch(`/updates/${id}.json`, { cache: "no-cache" })
      .then((r) => (r.ok ? r.json() : null))
      .then((m) => {
        if (!m || typeof m.url !== "string" || !m.url.startsWith("https://")) return false;
        show(id, m);
        if (id === "windows") showNotes(m);
        return true;
      })
      .catch(() => false),
  ),
).then((shown) => {
  if (!shown.some(Boolean)) {
    document.getElementById("none").hidden = false;
    return;
  }
  const mine = detect();
  if (mine && shown[platforms.indexOf(mine)]) recommend(mine);
});
