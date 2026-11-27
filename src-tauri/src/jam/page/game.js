// --- Multiplayer blind test (#441) ------------------------------------------------
// The game is run by the host: the page shows the round, sends an answer, then the result
// and the ranking. The track only arrives at the reveal.

/** Player id of this browser in the game, returned by the first answer. */
const PLAYER_KEY = `seaglass-jam-player:${location.pathname}`;
let player = null;
function loadPlayer() {
  try {
    const saved = JSON.parse(localStorage.getItem(PLAYER_KEY) || "null");
    player = saved && saved.code === code ? saved.id : null;
  } catch {
    player = null;
  }
}
/** Round this page has just answered, before the host confirms it. */
let answeredRound = 0;
/** Key of what is shown in `#game-play`: rebuilt only when it changes (otherwise the
 *  input in progress would be erased on every pushed state). */
let playKey = "";
let revealTimer = null;

function playerName(p) {
  if (p.host) return t.gameHost;
  return p.name || t.gameGuest(p.id);
}

function verdict(r) {
  if (!r.answered) return t.gameNoAnswer;
  if (r.title && r.artist) return t.gameBoth;
  if (r.title) return t.gameTitleOnly;
  if (r.artist) return t.gameArtistOnly;
  return t.gameMissed;
}

function renderGame(game) {
  const playing = Boolean(game);
  $("game").hidden = !playing;
  for (const id of ["search-form", "search-hint", "search-status", "tabs", "results"]) {
    if (playing) $(id).hidden = true;
  }
  if (!playing) {
    $("search-form").hidden = false;
    $("search-hint").hidden = false;
    $("results").hidden = false;
    if (playKey) {
      playKey = "";
      showResults();
    }
    clearInterval(revealTimer);
    return;
  }
  $("game-title").textContent = game.title ? `${t.game} · ${game.title}` : t.game;
  $("game-round").textContent = t.gameRound(game.round, game.rounds);
  const me = game.players.find((p) => p.id === player);
  const answered = answeredRound === game.round || Boolean(me?.answered);
  const key = `${game.round}:${game.phase}:${answered}`;
  if (key !== playKey) {
    playKey = key;
    $("game-play").replaceChildren(...gamePlay(game, answered));
  }
  showCountdown(game);
  renderBoard(game);
}

function gamePlay(game, answered) {
  if (game.phase === "finished") return [el("p", "game-status", t.gameFinished)];
  if (game.phase === "revealed" && game.last) return reveal(game.last);
  const nodes = [el("p", "game-status", answered ? t.gameSent : t.gameListen)];
  if (answered) return nodes;
  if (!name) nodes.push(el("p", "hint", t.gameNoName));
  if (game.mode === "choices") {
    const list = el("div", "choices");
    game.choices.forEach((choice, index) => {
      const button = el("button", "choice");
      button.type = "button";
      button.append(el("span", "title", choice.title), el("span", "sub", choice.artist));
      button.addEventListener("click", () => sendAnswer(game.round, { choice: index }, list));
      list.append(button);
    });
    nodes.push(list);
  } else {
    const form = el("form", "game-answer");
    form.autocomplete = "off";
    const input = el("input");
    input.maxLength = 200;
    input.placeholder = t.gamePlaceholder;
    input.setAttribute("aria-label", t.gamePlaceholder);
    input.enterKeyHint = "send";
    const send = el("button", "", t.gameSend);
    send.type = "submit";
    form.append(input, send);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      if (input.value.trim()) sendAnswer(game.round, { text: input.value }, form);
    });
    nodes.push(form);
    requestAnimationFrame(() => input.focus());
  }
  const pass = el("button", "link pass", t.gamePass);
  pass.type = "button";
  pass.addEventListener("click", () => sendAnswer(game.round, {}, pass));
  nodes.push(pass);
  return nodes;
}

async function sendAnswer(round, answer, block) {
  for (const node of [block, ...block.querySelectorAll("button, input")]) node.disabled = true;
  let res;
  try {
    res = await api("blind-answer", { round, name: name || undefined, ...answer });
  } catch (e) {
    for (const node of [block, ...block.querySelectorAll("button, input")]) node.disabled = false;
    if (lostAccess(e)) return;
    toast(message(e.kind));
    if (e.kind !== "round" && e.kind !== "answered") return;
  }
  if (res && typeof res.player === "number") {
    player = res.player;
    localStorage.setItem(PLAYER_KEY, JSON.stringify({ code, id: player }));
  }
  answeredRound = round;
  if (lastState) render(lastState);
}

function reveal(last) {
  const card = el("div", "game-reveal");
  card.append(cover(last.track.cover));
  const meta = el("div", "meta");
  meta.append(el("div", "title", last.track.title), el("div", "sub", last.track.artists));
  card.append(meta);
  const nodes = [card];
  const mine = last.players.find((p) => p.id === player);
  if (mine) {
    const line = el("p", `game-verdict${mine.points > 0 ? " good" : ""}`);
    line.append(el("strong", "", verdict(mine)), ` ${t.gamePoints(mine.points)}`);
    if (mine.fastest) line.append(` · ${t.gameFastest}`);
    nodes.push(line);
  }
  nodes.push(el("p", "muted", t.gameNext));
  return nodes;
}

function showCountdown(game) {
  clearInterval(revealTimer);
  const node = $("game-countdown");
  const tick = () => {
    const left = game.revealAt ? Math.ceil((game.revealAt - Date.now()) / 1000) : 0;
    node.hidden = game.phase !== "guess" || left <= 0;
    node.textContent = left > 0 ? t.gameRevealIn(left) : "";
  };
  tick();
  if (game.revealAt && game.phase === "guess") revealTimer = setInterval(tick, 1000);
}

function renderBoard(game) {
  const last = game.phase === "guess" ? null : game.last;
  const rows = game.players.map((p, i) => {
    const li = el("li", `player${p.id === player ? " me" : ""}`);
    li.append(el("span", "rank", String(i + 1)));
    const who = el("span", "player-name", playerName(p));
    if (p.id === player) who.append(el("span", "you", ` (${t.gameYou})`));
    li.append(who);
    const round = last?.players.find((r) => r.id === p.id);
    if (round) li.append(el("span", `gain${round.points > 0 ? " good" : ""}`, t.gamePoints(round.points)));
    else if (game.phase === "guess" && p.answered) li.append(el("span", "gain good", "✓"));
    li.append(el("span", "score", t.gameScore(p.score)));
    return li;
  });
  $("game-board").replaceChildren(...rows);
  $("game-board-title").hidden = rows.length === 0;
}
