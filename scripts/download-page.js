// Script de la page de téléchargement (download-page.html), publié à côté d'elle : la CSP du serveur
// interdit les scripts en ligne.
// Android et Linux sont publiés indépendamment de cette page : leur carte n'apparaît que
// si leur manifeste est en ligne et désigne une adresse HTTPS.
for (const id of ["android", "linux"]) {
  fetch(`${id}.json`, { cache: "no-cache" })
    .then((r) => (r.ok ? r.json() : null))
    .then((m) => {
      if (!m || typeof m.url !== "string" || !m.url.startsWith("https://")) return;
      document.getElementById(`${id}-link`).href = m.url;
      const size = (m.size / 1048576).toLocaleString("fr-FR", { maximumFractionDigits: 1 });
      document.getElementById(`${id}-meta`).textContent = `Version ${m.version} · ${size} Mo`;
      document.getElementById(id).hidden = false;
    })
    .catch(() => {});
}
