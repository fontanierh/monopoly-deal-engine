"use strict";
// Credentials live in the URL fragment, which browsers never send to the server.
const params = new URLSearchParams(location.hash.slice(1));
const seatToken = params.get("t");
const hostToken = params.get("host");
const $ = (id) => document.getElementById(id);

let catalog = {};
let state = null;
let selected = null; // card id whose actions are shown

const COLORS = {
  brown: "#8b5a2b", lightblue: "#9fd7f2", pink: "#d6449a", magenta: "#d6449a", orange: "#f39c32",
  red: "#e03b2f", yellow: "#f4d03f", green: "#2e9d4c", darkblue: "#1f4fa3", blue: "#1f4fa3",
  railroad: "#2b2b2b", railroads: "#2b2b2b", black: "#2b2b2b", utility: "#b7d36b", utilities: "#b7d36b",
};
const colorKey = (c) => String(c).toLowerCase().replace(/[^a-z]/g, "");
const colorCss = (c) => COLORS[colorKey(c)] || "#999";
const colorName = (c) => String(c).replace(/_/g, " ").replace(/\b\w/g, (m) => m.toUpperCase());

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined && text !== null) e.textContent = text;
  return e;
}

async function api(path, { token = seatToken, ...options } = {}) {
  const headers = { ...(options.headers || {}) };
  if (token) headers.Authorization = "Bearer " + token;
  const res = await fetch(path, { ...options, headers });
  const body = res.status === 204 ? null : await res.json();
  if (!res.ok) throw new Error(body && body.error ? body.error : res.statusText);
  return body;
}

// ---- cards -------------------------------------------------------------

const def = (id) => catalog[id] || { name: "Card #" + id, value: null, kind: null };
const cardName = (id) => shortName(def(id));

function kindColors(kind, out = []) {
  if (typeof kind === "string") {
    if (COLORS[colorKey(kind)]) out.push(kind);
  } else if (Array.isArray(kind)) {
    kind.forEach((k) => kindColors(k, out));
  } else if (kind && typeof kind === "object") {
    Object.values(kind).forEach((k) => kindColors(k, out));
  }
  return out;
}

function kindClass(kind) {
  const t = kind && kind.type;
  return t === "money" || t === "rent" || t === "action" ? t : "property";
}

const LIGHT = new Set(["lightblue", "yellow", "utility"]);
const SET_SIZE = { brown: 2, lightblue: 3, pink: 3, orange: 3, red: 3, yellow: 3, green: 3, darkblue: 2, railroad: 4, utility: 2 };

function shortName(d) {
  const k = d.kind || {};
  if (k.type === "wild") return k.colors.length > 2 ? "Wild: any color" : "Wild: " + k.colors.map(colorName).join("/");
  return d.name;
}

function subtitle(d) {
  const k = d.kind || {};
  switch (k.type) {
    case "property": return colorName(k.color) + " · set of " + SET_SIZE[colorKey(k.color)];
    case "wild": return "Property wild card";
    case "rent": return k.colors.length > 2 ? "Charge one player" : "All players pay";
    case "action": return "Action";
    default: return "";
  }
}

function stripes(colors) {
  if (colors.length === 1) return colorCss(colors[0]);
  const step = 100 / colors.length;
  return "linear-gradient(90deg," + colors.map((c, i) => colorCss(c) + " " + i * step + "% " + (i + 1) * step + "%").join(",") + ")";
}

function cardEl(id, { small = false, onClick = null, color = null } = {}) {
  const d = def(id);
  const k = d.kind || {};
  const kc = kindClass(k);
  const c = el("div", "card " + kc + (small ? " small" : ""));
  c.title = d.name + (d.value ? " ($" + d.value + "M)" : "");
  const head = el("div", "head", kc === "money" ? (small ? "$" + d.value + "M" : "Money") : shortName(d));
  const colors = color ? [color] : kindColors(k);
  if ((kc === "property" || kc === "rent") && colors.length) {
    head.style.background = stripes(colors);
    if (colors.length === 1 && LIGHT.has(colorKey(colors[0]))) head.classList.add("light");
  }
  if (kc === "money") head.style.background = "transparent";
  c.append(head);
  if (kc === "money") {
    c.append(el("div", "body", "$" + d.value + "M"));
  } else {
    const body = el("div", "body");
    body.append(el("span", null, subtitle(d)));
    c.append(body);
    if (d.value) c.append(el("div", "value", "$" + d.value + "M"));
  }
  if (onClick) {
    c.classList.add("clickable");
    if (selected === id) c.classList.add("selected");
    c.tabIndex = 0;
    c.setAttribute("role", "button");
    c.setAttribute("aria-label", d.name + (d.value ? ", $" + d.value + "M" : ""));
    c.onclick = onClick;
    c.onkeydown = (ev) => {
      if (ev.key === "Enter" || ev.key === " ") {
        ev.preventDefault();
        onClick();
      }
    };
  }
  return c;
}

// ---- names and descriptions -------------------------------------------

function findSet(setId) {
  for (const p of state.observation.players || []) {
    const s = (p.properties || []).find((x) => x.id === setId);
    if (s) return { owner: p.id, set: s };
  }
  return null;
}

function ownerSuffix(cardId) {
  for (const p of state.observation.players || []) {
    if ((p.properties || []).some((s) => s.cards.includes(cardId))) return " from " + playerName(p.id);
  }
  return "";
}

function newSetLabel(verb, color) {
  const me = (state.observation.players || []).find((p) => p.id === state.seat);
  const has = me && me.properties.some((s) => s.color === color);
  return verb + (has ? " a new " : " ") + colorName(color) + (has ? " set" : verb === "Play as" ? " property" : " set");
}

// Distinguishes several sets of one color owned by the same player.
function setOrdinal(ownerId, set) {
  const owner = state.observation.players.find((p) => p.id === ownerId);
  const same = owner.properties.filter((s) => s.color === set.color);
  return same.length > 1 ? " #" + (same.indexOf(set) + 1) : "";
}

function setLabel(setId, withOwner) {
  const f = findSet(setId);
  if (!f) return "a set";
  const color = colorName(f.set.color) + " set" + setOrdinal(f.owner, f.set);
  return withOwner ? (f.owner === state.seat ? "your " : playerName(f.owner) + "'s ") + color : color;
}

const playerName = (p) => (p === null || p === undefined ? "nobody" : p === state.seat ? "You" : "Player " + (p + 1));

function describeAction(a) {
  const c = a.card !== undefined ? cardName(a.card) : "";
  const who = (p) => playerName(p);
  switch (a.type) {
    case "bank": return "Put in bank ($" + (def(a.card).value ?? "?") + "M)";
    case "play_property": return a.set == null ? newSetLabel("Play as", a.color) : "Add to " + setLabel(a.set);
    case "play_pass_go": return "Play: draw 2 cards";
    case "play_birthday": return "Play: everyone pays you $2M";
    case "play_debt_collector": return "Collect $5M from " + who(a.target);
    case "play_rent": {
      const f = findSet(a.set);
      let s = "Charge " + setLabel(a.set) + " rent" + (f && f.set.rent != null ? " ($" + f.set.rent + "M)" : "");
      if (a.target !== null && a.target !== undefined) s += " to " + who(a.target);
      if (a.doubles && a.doubles.length) s += " (doubled ×" + a.doubles.length + ")";
      return s;
    }
    case "play_sly_deal": return "Sly Deal: take " + cardName(a.take) + ownerSuffix(a.take);
    case "play_forced_deal": return "Forced Deal: give " + cardName(a.give) + " for " + cardName(a.take) + ownerSuffix(a.take);
    case "play_deal_breaker": return "Deal Breaker: take " + setLabel(a.set, true);
    case "play_house": return "Add house to " + setLabel(a.set);
    case "play_hotel": return "Add hotel to " + setLabel(a.set);
    case "move_property": return a.set == null ? newSetLabel("Move to", a.color) : "Move to " + setLabel(a.set);
    case "end_turn": return "End turn";
    case "discard": return "Discard " + c;
    case "just_say_no": return "Just Say No!";
    case "accept": return "Accept";
    case "pay_card": return "Pay with " + c + " ($" + (def(a.card).value ?? 0) + "M)";
    default: return describeGeneric(a);
  }
}

function describeGeneric(obj) {
  if (typeof obj !== "object" || obj === null) return String(obj);
  const { type, ...rest } = obj;
  const parts = Object.entries(rest).map(([k, v]) => {
    if (/^(player|target|payer|to|from|responder|winner|by)$/.test(k) && typeof v === "number") return k + ": " + playerName(v);
    if (/^(card|take|give)$/.test(k) && typeof v === "number") return k + ": " + cardName(v);
    if (/cards$/.test(k) && Array.isArray(v)) return k + ": " + v.map(cardName).join(", ");
    if (k === "color") return colorName(v);
    if (k === "set" && typeof v === "number") return setLabel(v, true);
    return k + ": " + (typeof v === "object" ? JSON.stringify(v) : v);
  });
  return colorName(type || "event") + (parts.length ? " — " + parts.join(", ") : "");
}

function phaseText(ph) {
  if (!ph) return "";
  const mine = (p) => p === state.seat;
  switch (ph.type) {
    case "play": return (mine(ph.player) ? "Your turn" : playerName(ph.player) + "'s turn") + " · " + ph.plays_left + " play" + (ph.plays_left === 1 ? "" : "s") + " left";
    case "respond": return playerName(ph.actor) + " played " + cardName(ph.card) + " on " + (mine(ph.target) ? "you" : playerName(ph.target)) + " · " + (mine(ph.responder) ? "your response" : "waiting for " + playerName(ph.responder));
    case "pay": return (mine(ph.payer) ? "You owe " : playerName(ph.payer) + " owes ") + (mine(ph.to) ? "you" : playerName(ph.to)) + " $" + ph.owed + "M (paid $" + ph.paid + "M)";
    case "discard": return (mine(ph.player) ? "You must" : playerName(ph.player) + " must") + " discard " + ph.excess;
    case "game_over": return mine(ph.winner) ? "You win!" : playerName(ph.winner) + " wins!";
    default: return describeGeneric(ph);
  }
}

function effectText(ph) {
  const e = ph.effect || {};
  const you = ph.actor === state.seat;
  const actor = playerName(ph.actor);
  const want = you ? " want to " : " wants to ";
  const nojsn = ph.just_say_no_count ? " (Just Say No played " + ph.just_say_no_count + "×)" : "";
  switch (e.type) {
    case "charge": return actor + (you ? " charge " : " charges ") + (ph.target === state.seat ? "you" : playerName(ph.target)) + " $" + e.amount + "M." + nojsn;
    case "sly_deal": return actor + want + "take " + cardName(e.take) + "." + nojsn;
    case "forced_deal": return actor + want + "swap " + cardName(e.give) + " for " + cardName(e.take) + "." + nojsn;
    case "deal_breaker": return actor + want + "take " + setLabel(e.set, true) + "." + nojsn;
    default: return actor + " played " + cardName(ph.card) + "." + nojsn;
  }
}

function waitingText(obs) {
  if (obs.phase.type === "game_over") return "";
  const waiting = (obs.pending_actors || []).map(playerName);
  return waiting.length ? "Waiting for " + waiting.join(", ") + "…" : "";
}

const TAP = matchMedia("(pointer: coarse)").matches ? "Tap" : "Click";

function promptText(ph) {
  switch (ph && ph.type) {
    case "play": return TAP + " a highlighted card to play it, or end your turn.";
    case "respond": return effectText(ph) + " Accept, or answer with Just Say No.";
    case "pay": return "Pay $" + (ph.owed - ph.paid) + "M more. " + TAP + " bank or property cards one at a time.";
    case "discard": return "Too many cards. " + TAP + " cards in your hand to discard " + ph.excess + " more.";
    default: return "Your move.";
  }
}

// ---- rendering --------------------------------------------------------

function actionsByCard(actions) {
  const map = new Map();
  for (const a of actions) {
    if (a.card === undefined) continue;
    if (!map.has(a.card)) map.set(a.card, []);
    map.get(a.card).push(a);
  }
  return map;
}

function clickFor(byCard, id) {
  const acts = byCard.get(id);
  if (!acts) return null;
  return () => {
    // A lone action on a card is played directly; otherwise show the choices.
    if (acts.length === 1 && state.observation.phase.type !== "play") return send(acts[0]);
    selected = selected === id ? null : id;
    render(state);
  };
}

function targetedCards(ph) {
  const e = (ph && ph.type === "respond" && ph.effect) || {};
  const ids = [e.take, e.give].filter((x) => x != null);
  if (e.set != null) {
    const f = findSet(e.set);
    if (f) ids.push(...f.set.cards);
  }
  return new Set(ids);
}

function playerPanel(p, obs, byCard) {
  const isMe = p.id === state.seat;
  const targeted = targetedCards(obs.phase);
  const click = (id) => (isMe ? clickFor(byCard, id) : null);
  const panel = el("div", "player");
  if (p.id === obs.current_player) panel.classList.add("turn");
  if ((obs.pending_actors || []).includes(p.id)) panel.classList.add("acting");

  const head = el("div", "player-head");
  const name = el("span", "player-name", isMe ? "You · Player " + (p.id + 1) : playerName(p.id));
  if (p.id === obs.current_player && obs.phase.type !== "game_over") name.append(el("span", "tag", isMe ? "Your turn" : "Turn"));
  head.append(name);
  const meta = el("span", "player-meta");
  const backs = el("span", "hand-backs");
  for (let i = 0; i < Math.min(p.hand_count, 10); i++) backs.append(el("div", "card back"));
  const stat = (label, value) => {
    const s = el("span", "stat", label + " ");
    s.append(el("b", null, value));
    return s;
  };
  meta.append(backs, stat("Hand", p.hand_count), stat("Bank", "$" + p.bank_value + "M"), stat("Sets", p.complete_colors + "/3"));
  head.append(meta);
  panel.append(head);

  const zones = el("div", "zones");
  const props = el("div", "zone props");
  props.append(el("div", "zone-label", "Properties"));
  const setsEl = el("div", "sets");
  for (const s of p.properties || []) {
    const col = el("div", "set" + (s.complete ? " complete" : ""));
    const sh = el("div", "set-head");
    const ordinal = setOrdinal(p.id, s);
    sh.append(
      el("span", "set-color" + (ordinal ? " dup" : ""), colorName(s.color) + ordinal),
      el("span", null, s.cards.length + "/" + (SET_SIZE[colorKey(s.color)] || "?") + (s.complete ? " ✓" : "")),
      el("span", null, "rent $" + s.rent + "M"),
    );
    const cards = el("div", "set-cards");
    for (const id of s.cards) {
      const c = cardEl(id, { color: s.color, onClick: click(id) });
      if (targeted.has(id)) c.classList.add("targeted");
      cards.append(c);
    }
    for (const id of [s.house, s.hotel]) if (id != null) cards.append(cardEl(id, { onClick: click(id) }));
    col.append(sh, cards);
    setsEl.append(col);
  }
  if (!setsEl.childElementCount) setsEl.append(el("span", "empty", "No properties yet"));
  props.append(setsEl);

  const bankZone = el("div", "zone bankzone");
  bankZone.append(el("div", "zone-label", "Bank"));
  const bank = el("div", "bank");
  for (const id of p.bank || []) bank.append(cardEl(id, { small: true, onClick: click(id) }));
  if (!bank.childElementCount) bank.append(el("span", "empty", "Empty"));
  bankZone.append(bank);
  zones.append(props, bankZone);
  panel.append(zones);
  return panel;
}

function describeEvent(e) {
  const who = (x) => playerName(x);
  const whom = (x) => (x === state.seat ? "you" : playerName(x));
  const n = (x) => cardName(x);
  switch (e.type) {
    case "turn_started": return "Turn " + e.turn + ": " + who(e.player);
    case "drew": return who(e.player) + " drew " + e.count;
    case "draw_pile_reshuffled": return "Played cards reshuffled into the draw pile (" + e.count + ")";
    case "banked": return who(e.player) + " banked " + n(e.card) + " ($" + def(e.card).value + "M)";
    case "property_played": return who(e.player) + " played " + n(e.card) + " as " + colorName(e.color);
    case "property_moved": return who(e.player) + " moved " + n(e.card) + " to " + colorName(e.color);
    case "pass_go": return who(e.player) + " played Pass Go";
    case "action_played": {
      const t = e.targets && e.targets.length ? " on " + e.targets.map(whom).join(", ") : "";
      const d = e.doubles && e.doubles.length ? " (doubled ×" + e.doubles.length + ")" : "";
      return who(e.player) + " played " + n(e.card) + t + d;
    }
    case "building_placed": return who(e.player) + " added " + n(e.card) + " to " + setLabel(e.set, true);
    case "just_say_no": return who(e.player) + " said Just Say No!";
    case "accepted": return who(e.player) + " accepted";
    case "action_cancelled": return who(e.actor) + "'s action on " + whom(e.target) + " was cancelled";
    case "property_taken": return who(e.to) + " took " + n(e.card) + " from " + whom(e.from);
    case "set_taken": return who(e.to) + " took " + setLabel(e.set) + " from " + whom(e.from);
    case "paid": return who(e.from) + " paid " + whom(e.to) + " with " + n(e.card) + " ($" + def(e.card).value + "M)";
    case "payment_finished": return who(e.from) + " paid " + whom(e.to) + " $" + e.paid + "M in total (owed $" + e.owed + "M)";
    case "turn_ended": return who(e.player) + " ended the turn";
    case "discarded_to_draw_pile": return who(e.player) + " discarded a card face down";
    case "game_won": return who(e.winner) + " won the game!";
    default: return describeGeneric(e);
  }
}

function render(s) {
  state = s;
  const obs = s.observation;
  const actions = s.actions || [];
  const byCard = actionsByCard(actions);
  if (selected !== null && !byCard.has(selected)) selected = null;

  $("who").textContent = s.seat === null ? "Spectator" : "Player " + (s.seat + 1) + " of " + s.seats;
  const ph = $("phase");
  ph.textContent = phaseText(obs.phase);
  ph.classList.toggle("mine", actions.length > 0);
  document.title = (actions.length ? "● " : "") + "Monopoly Deal";

  // Seats after mine come first, so play order reads left to right.
  const players = obs.players || [];
  const start = s.seat === null ? 0 : s.seat + 1;
  const ordered = players.slice(start).concat(players.slice(0, start)).filter((p) => p.id !== s.seat);
  $("opponents").replaceChildren(...ordered.map((p) => playerPanel(p, obs, byCard)));
  const me = players.find((p) => p.id === s.seat);
  $("me").replaceChildren(...(me ? [playerPanel(me, obs, byCard)] : []));

  $("draw-count").textContent = obs.draw_count;
  const discard = obs.discard || [];
  $("discard-top").replaceChildren(discard.length ? cardEl(discard[discard.length - 1]) : el("div", "card placeholder"));
  $("discard-label").textContent = "Discard (" + discard.length + ")";
  $("turn-info").textContent = "Turn " + obs.turn;

  // While paying, the dock offers the table cards that can pay instead of the hand.
  const hand = obs.hand || [];
  const payWith = actions.filter((a) => a.type === "pay_card").map((a) => a.card);
  const strip = payWith.length ? payWith : hand;
  $("dock").hidden = s.seat === null;
  $("hand-title").textContent = payWith.length ? "Pay with" : "Your Hand (" + hand.length + ")";
  $("hand").replaceChildren(...strip.map((id) => cardEl(id, { onClick: clickFor(byCard, id) })));
  if (s.seat !== null && !strip.length) $("hand").append(el("span", "empty", "no cards"));

  $("prompt").textContent = actions.length ? promptText(obs.phase) : waitingText(obs);
  const global = actions.filter((a) => a.card === undefined || a.type === "just_say_no");
  $("global-actions").replaceChildren(...global.map((a) => {
    const b = el("button", a.type === "just_say_no" ? "danger big" : "primary big", describeAction(a) + (a.type === "end_turn" ? " ›" : ""));
    b.onclick = () => send(a);
    return b;
  }));
  $("card-actions").hidden = selected === null;
  if (selected !== null) {
    const cancel = el("button", "cancel", "Cancel");
    cancel.onclick = () => {
      selected = null;
      render(state);
    };
    $("card-actions").replaceChildren(el("span", "card-actions-title", shortName(def(selected)) + ":"), cancel, ...byCard.get(selected).map((a) => {
      const b = el("button", a.type === "just_say_no" ? "danger" : "", describeAction(a));
      b.onclick = () => send(a);
      return b;
    }));
  }

  requestAnimationFrame(markOverflow);
  $("roster").replaceChildren(...players.map((p) => {
    const li = el("li", p.id === s.seat ? "me" : null);
    li.append(el("span", null, playerName(p.id)), el("span", null, p.hand_count + " cards · $" + p.bank_value + "M · " + p.complete_colors + "/3 sets"));
    return li;
  }));
  $("log").replaceChildren(...(obs.log || []).slice().reverse().map((e) => el("li", e.type === "turn_started" ? "turn" : null, describeEvent(e))));
  renderPhone(s, byCard);
}

async function send(action) {
  $("error").textContent = $("m-error").textContent = "";
  try {
    selected = null;
    render(await api("/api/action", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ version: state.version, action }),
    }));
  } catch (e) {
    $("error").textContent = $("m-error").textContent = e.message;
  }
}

async function poll() {
  for (;;) {
    try {
      render(await api("/api/state" + (state ? "?since=" + state.version : "")));
      $("conn").textContent = $("m-conn").textContent = "Live";
      $("conn").classList.remove("bad");
      $("m-conn").classList.remove("bad");
    } catch (e) {
      $("conn").textContent = "offline: " + e.message;
      $("m-conn").textContent = "Offline";
      $("conn").classList.add("bad");
      $("m-conn").classList.add("bad");
      await new Promise((r) => setTimeout(r, 2000));
    }
  }
}

const TABS = { "tab-log": "log", "tab-players": "roster" };
for (const tab of Object.keys(TABS)) {
  $(tab).onclick = () => {
    for (const [t, panel] of Object.entries(TABS)) {
      $(t).classList.toggle("active", t === tab);
      $(t).setAttribute("aria-selected", String(t === tab));
      $(panel).hidden = t !== tab;
    }
  };
}

// Fade the right edge of horizontal strips that have more cards off screen.
function markOverflow() {
  for (const strip of document.querySelectorAll(".sets, .bank, .hand, .m-hand, .m-strip, .m-choices, .m-chips")) {
    const more = strip.scrollWidth - strip.scrollLeft - strip.clientWidth > 2;
    strip.classList.toggle("fade-right", more);
  }
}
document.addEventListener("scroll", markOverflow, { capture: true, passive: true });
window.addEventListener("resize", markOverflow);

if (hostToken) {
  $("new-game").hidden = false;
  $("new-game").onclick = async () => {
    if (!confirm("Start a new game? The current game will be lost.")) return;
    try { await api("/api/new-game", { method: "POST", token: hostToken }); } catch (e) { alert(e.message); }
  };
}

api("/api/cards").then((c) => { catalog = c; poll(); }, (e) => { $("conn").textContent = "offline: " + e.message; });
