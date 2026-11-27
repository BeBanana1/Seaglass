// --- Startup -----------------------------------------------------------------

$("join-form").addEventListener("submit", (event) => {
  event.preventDefault();
  join($("code").value);
});
$("forget").addEventListener("click", () => {
  localStorage.removeItem(CODE_KEY);
  code = "";
  showJoin();
});
$("query").placeholder = t.searchPlaceholder;
$("query").addEventListener("input", scheduleSearch);
$("search-form").addEventListener("submit", (event) => {
  event.preventDefault();
  clearTimeout(searchTimer);
  const query = $("query").value.trim();
  if (query) search(query);
  $("query").blur();
});
$("clear").addEventListener("click", () => {
  $("query").value = "";
  scheduleSearch();
  $("query").focus();
});
for (const name of ["queue", "search", "playlists"]) {
  $(`tab-${name}`).addEventListener("click", () => showTab(name));
}
setupPlayer();
showTab("queue");

// The app's QR code: the code comes after `#`, never sent to the server. Removed from the
// address so it does not stay in the history. A malformed fragment is ignored.
let fromQr = "";
try {
  fromQr = decodeURIComponent(location.hash.slice(1));
} catch {
  // Unreadable fragment: nothing to pick up.
}
if (location.hash) history.replaceState(null, "", location.pathname);
if (fromQr) code = fromQr;

if (code) join(code);
else showJoin();
