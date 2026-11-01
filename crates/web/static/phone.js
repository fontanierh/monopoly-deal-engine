"use strict";
// Phone layout. Shares state and helpers with app.js: one player's table as a
// compact strip, a stage that explains the selected card, horizontal choice
// tiles (also tappable as targets on the table), and a fixed hand dock.

let focusPlayer = null; // whose table the strip shows
let partial = {}; // fields already chosen for the selected card, e.g. { take: "12" }

const KIND_LABEL = { money: "Money", property: "Property", rent: "Rent", action: "Action" };
const seatName = (p) => (p === state.seat ? "You" : "Player " + (p + 1));
const plays = (n) => n + " play" + (n === 1 ? "" : "s");

function selectedActions() {
  return selected === null ? null : actionsByCard(state.actions).get(selected) || null;
}

// Choices are narrowed one field at a time, using only the legal action list.
const DIMS = ["type", "take", "color", "set", "target", "give", "doubles"];
const fieldValue = (a, f) => (f === "doubles" ? (a.doubles || []).length : a[f] ?? null);
const fieldKey = (a, f) => JSON.stringify(fieldValue(a, f));

function remaining() {
  return (selectedActions() || []).filter((a) => Object.entries(partial).every(([f, k]) => fieldKey(a, f) === k));
}
function nextField(rem) {
  return DIMS.find((f) => new Set(rem.map((a) => fieldKey(a, f))).size > 1) || null;
}
function chosen() {
  const rem = remaining();
  return rem.length && !nextField(rem) ? rem[0] : null;
}

// Table cards that stand for each value of a card-like field.
function fieldCards(a, f) {
  if (f === "take" || f === "give") return [a[f]];
  if (f === "set") {
    const found = findSet(a.set);
    return found ? found.set.cards : [];
  }
  return [];
}

function ownerOf(cardId) {
  for (const p of state.observation.players) if (p.properties.some((s) => s.cards.includes(cardId))) return p.id;
  return null;
}

function valueOwner(a, f) {
  if (f === "target") return a.target;
  const cards = fieldCards(a, f);
  return cards.length ? ownerOf(cards[0]) : null;
}

// If every option for the next field sits with one player, show that player.
function autoFocus() {
  const rem = remaining();
  const f = nextField(rem);
  if (!f) return;
  // Show the player with the most options unless the current table has some.
  const counts = new Map();
  for (const a of rem) {
    const o = valueOwner(a, f);
    if (o !== null) counts.set(o, (counts.get(o) || 0) + 1);
  }
  if (counts.size && !counts.has(focusPlayer)) focusPlayer = [...counts].sort((x, y) => y[1] - x[1])[0][0];
}

function choose(f, key) {
  partial = { ...partial, [f]: key };
  autoFocus();
  render(state);
}

function back() {
  const keys = Object.keys(partial);
  if (!keys.length) return selectCard(selected);
  const rest = { ...partial };
  delete rest[keys[keys.length - 1]];
  partial = rest;
  render(state);
}

function selectCard(id) {
  selected = selected === id ? null : id;
  partial = {};
  if (selected !== null) autoFocus();
  render(state);
}

function verb(a) {
  switch (a.type) {
    case "bank": return "Bank $" + def(a.card).value + "M";
    case "pay_card": return "Pay $" + def(a.card).value + "M";
    case "discard": return "Discard";
    case "just_say_no": return "Just Say No!";
    case "move_property": return "Move card";
    default: return "Play card";
  }
}

const FIELD_PROMPT = {
  type: "Choose how to use this card",
  take: "Tap a property to take",
  color: "Choose a color",
  set: "Choose a set",
  target: "Choose a player",
  give: "Tap one of your properties to give",
  doubles: "Add Double the Rent?",
};

// ---- small pieces ------------------------------------------------------

function mini(id, { color = null, cls = "", onClick = null, label = null } = {}) {
  const d = def(id);
  const kc = kindClass(d.kind);
  const t = el(onClick ? "button" : "div", "mini " + kc + (cls ? " " + cls : ""));
  if (kc === "money") {
    t.append(el("span", "mini-money", "$" + d.value + "M"));
  } else {
    // Wild and rent cards show their colors in the band, so the name stays short.
    const k = d.kind || {};
    const band = el("span", "mini-band");
    const colors = k.type === "wild" || k.type === "rent" ? kindColors(k) : color ? [color] : kindColors(k);
    band.style.background = kc === "action" ? "#c0392b" : colors.length ? stripes(colors) : "#6c3483";
    const name = k.type === "wild" ? "Wild" : k.type === "rent" ? "Rent" : d.name;
    t.append(band, el("span", "mini-name", name));
    if (k.type === "wild" || k.type === "rent") {
      // Zero-width spaces let "Light Blue/Railroad" wrap after the slash.
      const sub = k.colors.length > 2 ? "Any color" : k.colors.map(colorName).join("/\u200b");
      t.append(el("span", "mini-sub", sub));
    }
    if (d.value) t.append(el("span", "mini-val", "$" + d.value + "M"));
  }
  t.title = d.name;
  if (onClick) {
    t.type = "button";
    t.onclick = onClick;
    t.setAttribute("aria-label", label || d.name);
  }
  return t;
}

function swatch(color) {
  const s = el("span", "swatch");
  s.style.background = colorCss(color);
  return s;
}

// Scroll a horizontal strip (only) so that `item` is visible.
function scrollStripTo(item) {
  if (!item) return;
  const strip = item.closest(".m-chips, .m-strip, .m-choices, .m-hand");
  const left = item.offsetLeft - strip.offsetLeft;
  if (left < strip.scrollLeft || left + item.offsetWidth > strip.scrollLeft + strip.clientWidth) {
    strip.scrollTo({ left: Math.max(0, left - 16), behavior: "smooth" });
  }
}

// ---- chips -------------------------------------------------------------

function chipSub(p, obs) {
  const ph = obs.phase;
  if (p.id === state.seat && state.actions.length) {
    if (ph.type === "play") return plays(ph.plays_left) + " left";
    if (ph.type === "respond") return "Respond now";
    if (ph.type === "pay") return "Pay $" + (ph.owed - ph.paid) + "M";
    if (ph.type === "discard") return "Discard " + ph.excess;
  }
  return p.hand_count + " cards · $" + p.bank_value + "M";
}

// How many options for the next field belong to each player.
function optionCounts() {
  const counts = new Map();
  const rem = remaining();
  const f = nextField(rem);
  if (!f) return counts;
  const seen = new Set();
  for (const a of rem) {
    const k = fieldKey(a, f);
    if (seen.has(k)) continue;
    seen.add(k);
    const o = valueOwner(a, f);
    if (o !== null) counts.set(o, (counts.get(o) || 0) + 1);
  }
  return counts;
}

function renderChips(obs) {
  const seat = state.seat ?? -1;
  const ordered = obs.players.filter((p) => p.id === seat).concat(obs.players.filter((p) => p.id > seat), obs.players.filter((p) => p.id < seat));
  const counts = optionCounts();
  $("m-chips").replaceChildren(...ordered.map((p) => {
    const b = el("button", "m-chip" + (p.id === focusPlayer ? " on" : "") + ((obs.pending_actors || []).includes(p.id) ? " acting" : ""));
    b.type = "button";
    b.setAttribute("role", "tab");
    b.setAttribute("aria-selected", String(p.id === focusPlayer));
    const text = el("span", "m-chip-text");
    text.append(el("strong", null, seatName(p.id)), el("small", null, chipSub(p, obs)));
    b.append(el("span", "m-dot"), text);
    const n = counts.get(p.id);
    if (n) {
      const badge = el("span", "m-badge", String(n));
      badge.setAttribute("aria-label", n + " option" + (n === 1 ? "" : "s"));
      b.append(badge);
    }
    b.onclick = () => {
      focusPlayer = p.id;
      render(state);
    };
    return b;
  }));
  scrollStripTo($("m-chips").querySelector(".on"));
}

// ---- the table strip ---------------------------------------------------

function renderStrip(obs, byCard) {
  const p = obs.players.find((x) => x.id === focusPlayer);
  const mine = p.id === state.seat;
  $("m-table-label").replaceChildren(
    el("strong", null, mine ? "Your table" : seatName(p.id) + "'s table"),
    el("span", null, "Bank $" + p.bank_value + "M · Sets " + p.complete_colors + "/3"),
  );
  const rem = remaining();
  const f = selected !== null ? nextField(rem) : null;
  const pickingCards = f === "take" || f === "give" || f === "set";
  const options = new Map(); // card id -> [field, key, example action]
  if (pickingCards) for (const a of rem) for (const id of fieldCards(a, f)) if (!options.has(id)) options.set(id, [f, fieldKey(a, f), a]);
  const picked = new Set();
  for (const [field, key] of Object.entries(partial)) {
    const a = rem.find((x) => fieldKey(x, field) === key);
    if (a) fieldCards(a, field).forEach((id) => picked.add(id));
  }
  const done = chosen();
  if (done) for (const field of ["take", "give", "set"]) fieldCards(done, field).forEach((id) => id != null && picked.add(id));
  const threatened = new Set(selected === null ? targetedCards(obs.phase) : []);

  const tile = (id, color) => {
    let onClick = null;
    let label = null;
    let cls = "";
    if (options.has(id)) {
      const [field, key, a] = options.get(id);
      onClick = () => choose(field, key);
      label = FIELD_PROMPT[field] + ": " + (field === "set" ? setLabel(a.set, true) : cardName(id));
      cls = "option";
    } else if (pickingCards) {
      cls = "dim";
    } else if (mine && byCard.has(id) && selected === null) {
      onClick = () => selectCard(id);
      cls = "ready";
    }
    if (picked.has(id) || selected === id) cls += " chosen";
    if (threatened.has(id)) cls += " threatened";
    return mini(id, { color, cls, onClick, label });
  };

  const groups = [];
  for (const s of p.properties) {
    const g = el("div", "m-group" + (s.complete ? " complete" : ""));
    g.title = colorName(s.color) + setOrdinal(p.id, s) + ": " + s.cards.length + " of " + SET_SIZE[colorKey(s.color)] + ", rent $" + s.rent + "M";
    const cap = el("span", "m-group-cap");
    cap.append(swatch(s.color), document.createTextNode(colorName(s.color) + setOrdinal(p.id, s) + " " + s.cards.length + "/" + SET_SIZE[colorKey(s.color)] + (s.complete ? " ✓" : "")));
    const row = el("div", "m-group-row");
    for (const id of s.cards) row.append(tile(id, s.color));
    for (const id of [s.house, s.hotel]) if (id != null) row.append(tile(id, null));
    g.append(cap, row);
    groups.push(g);
  }
  if (p.bank.length) {
    const g = el("div", "m-group bank");
    const row = el("div", "m-group-row");
    for (const id of p.bank) row.append(tile(id, null));
    g.append(el("span", "m-group-cap", "Bank"), row);
    groups.push(g);
  }
  if (!groups.length) groups.push(el("p", "m-strip-empty", mine ? "Nothing on your table yet." : "Nothing on the table yet."));
  $("m-strip").replaceChildren(...groups);
  scrollStripTo($("m-strip").querySelector(".option, .chosen"));
}

// ---- stage and choices -------------------------------------------------

function respondText(ph) {
  const card = cardName(ph.card);
  if (ph.just_say_no_count % 2 === 1) {
    return (ph.responder === state.seat ? seatName(ph.target) + " said Just Say No! to your " : seatName(ph.target) + " said Just Say No! to " + seatName(ph.actor) + "'s ") + card + ".";
  }
  const again = ph.just_say_no_count ? " " + seatName(ph.actor) + " said Just Say No! back." : "";
  return effectText(ph) + again;
}

function idleStage(obs) {
  const ph = obs.phase;
  const mineNow = state.actions.length > 0;
  let eyebrow;
  let title;
  let text = "";
  if (state.seat === null) {
    eyebrow = "Watching";
    title = phaseText(ph);
    text = "Hands stay hidden. Tap a player to see their table.";
  } else if (ph.type === "game_over") {
    eyebrow = "Game over";
    title = ph.winner === state.seat ? "You win!" : seatName(ph.winner) + " wins";
  } else if (!mineNow) {
    eyebrow = "Waiting";
    title = phaseText(ph);
    text = ph.type === "respond" ? respondText(ph) : waitingText(obs);
  } else if (ph.type === "play") {
    eyebrow = "Your turn";
    title = plays(ph.plays_left) + " left";
    text = "Tap a card in your hand to see what it can do.";
  } else if (ph.type === "respond") {
    eyebrow = "Respond";
    title = respondText(ph);
    text = state.actions.some((a) => a.type === "just_say_no") ? "Accept it, or cancel it with Just Say No!" : "You have no Just Say No!, so accept to continue.";
  } else if (ph.type === "pay") {
    eyebrow = "Pay " + seatName(ph.to);
    title = "$" + ph.paid + "M of $" + ph.owed + "M paid";
    text = "Tap a bank or table card below, one at a time. No change is given.";
  } else {
    eyebrow = "Discard";
    title = "Discard " + ph.excess + " more";
    text = "You may keep 7 cards. Tap a card in your hand.";
  }
  const box = el("div", "m-stage-text");
  box.append(el("p", "m-eyebrow", eyebrow), el("h2", null, title));
  if (text) box.append(el("p", "m-desc", text));
  if (ph.type === "pay" && mineNow) {
    const bar = el("div", "m-progress");
    const fill = el("span");
    fill.style.width = Math.min(100, (100 * ph.paid) / ph.owed) + "%";
    bar.append(fill);
    box.append(bar);
  }
  return [box];
}

function pickedStage() {
  const d = def(selected);
  const done = chosen();
  const rem = remaining();
  const box = el("div", "m-stage-text");
  const top = el("div", "m-stage-top");
  const nav = el("div", "m-stage-nav");
  const title = el("h2", null, shortName(d));
  title.append(el("span", "m-kind", KIND_LABEL[kindClass(d.kind)]));
  top.append(title, nav);
  box.append(top);
  box.append(el("p", "m-desc", done ? describeAction(done) : FIELD_PROMPT[nextField(rem)] + "."));
  if (Object.keys(partial).length) {
    const b = el("button", "m-link", "‹ Back");
    b.type = "button";
    b.onclick = back;
    nav.append(b);
  }
  const close = el("button", "m-link", "Cancel");
  close.type = "button";
  close.onclick = () => selectCard(selected);
  nav.append(close);
  return [box];
}

function optionLabel(a, f) {
  switch (f) {
    case "type":
      return { bank: "Bank $" + def(a.card).value + "M", move_property: "Move", play_property: "Play property" }[a.type] || "Play";
    case "take":
    case "give":
      return cardName(a[f]);
    case "color":
      return colorName(a.color);
    case "set": {
      if (a.set == null) return "New set";
      const found = findSet(a.set);
      if (!found) return "Set";
      const owner = found.owner === state.seat ? "Your " : seatName(found.owner) + "'s ";
      return owner + colorName(found.set.color) + setOrdinal(found.owner, found.set) + (a.type === "play_rent" ? " · $" + found.set.rent + "M" : "");
    }
    case "target":
      return a.target == null ? "Everyone" : seatName(a.target);
    case "doubles":
      return a.doubles.length ? "Double ×" + Math.pow(2, a.doubles.length) : "No double";
    default:
      return describeAction(a);
  }
}

function setColorOf(cardId) {
  for (const p of state.observation.players) for (const s of p.properties) if (s.cards.includes(cardId)) return s.color;
  return null;
}

function optionArt(a, f) {
  const art = el("span", "m-choice-art");
  if (f === "type" && a.type === "bank") art.append(el("span", "m-cash", "$" + def(a.card).value + "M"));
  else if (f === "take" || f === "give") art.append(swatch(setColorOf(a[f])));
  else if (f === "color") art.append(swatch(a.color));
  else if (f === "set" && findSet(a.set)) art.append(swatch(findSet(a.set).set.color));
  else if (f === "target") art.append(el("span", "m-cash player", a.target == null ? "All" : "P" + (a.target + 1)));
  else art.append(el("span", "m-cash play", f === "doubles" ? "×" + Math.pow(2, a.doubles.length) : "▶"));
  return art;
}

function renderChoices() {
  const list = $("m-choices");
  const rem = remaining();
  const f = selected !== null ? nextField(rem) : null;
  list.hidden = !f;
  if (!f) return;
  list.setAttribute("aria-label", FIELD_PROMPT[f]);
  const seen = new Map();
  for (const a of rem) if (!seen.has(fieldKey(a, f))) seen.set(fieldKey(a, f), a);
  list.replaceChildren(...[...seen].map(([key, a]) => {
    const b = el("button", "m-choice");
    b.type = "button";
    b.append(optionArt(a, f), el("span", "m-choice-label", optionLabel(a, f)));
    if (f === "take") b.append(el("small", "m-choice-sub", seatName(ownerOf(a.take))));
    b.onclick = () => choose(f, key);
    return b;
  }));
}

// ---- dock --------------------------------------------------------------

function renderDock(obs, byCard) {
  const s = state;
  const payWith = s.actions.filter((a) => a.type === "pay_card").map((a) => a.card);
  const hand = obs.hand || [];
  const strip = payWith.length ? payWith : hand;
  $("m-hand-title").textContent = payWith.length ? "Pay with" : "Your hand (" + hand.length + ")";
  $("m-hand").replaceChildren(...strip.map((id) => {
    const playable = byCard.has(id);
    const cls = "hand" + (selected === id ? " chosen" : playable ? " ready" : s.actions.length ? " idle" : "");
    return mini(id, { cls, onClick: playable ? () => selectCard(id) : null, label: def(id).name + (def(id).value ? ", $" + def(id).value + "M" : "") });
  }));
  if (!strip.length) $("m-hand").append(el("span", "m-strip-empty", "No cards"));

  const bar = $("m-bar");
  if (selected !== null) {
    const done = chosen();
    const go = el("button", done && done.type === "just_say_no" ? "danger" : "primary", done ? verb(done) : FIELD_PROMPT[nextField(remaining())]);
    go.disabled = !done;
    if (done) go.onclick = () => send(done);
    bar.replaceChildren(go);
    return;
  }
  const jsn = s.actions.find((x) => x.type === "just_say_no");
  const global = s.actions.filter((x) => x.card === undefined).concat(jsn ? [jsn] : []);
  bar.replaceChildren(...global.map((x) => {
    let label = describeAction(x);
    if (x.type === "end_turn" && obs.phase.plays_left) label += " · " + plays(obs.phase.plays_left) + " unused";
    const b = el("button", x.type === "just_say_no" ? "danger" : "primary", label);
    b.onclick = () => send(x);
    return b;
  }));
}

function renderPhone(s, byCard) {
  const obs = s.observation;
  if (selected === null) partial = {};
  if (focusPlayer === null || !obs.players.some((p) => p.id === focusPlayer)) focusPlayer = s.seat ?? obs.current_player;
  $("m-status").textContent = phaseText(obs.phase);
  $("m-status").classList.toggle("mine", s.actions.length > 0);
  renderChips(obs);
  renderStrip(obs, byCard);
  const stage = $("m-stage");
  stage.classList.toggle("picked", selected !== null);
  stage.replaceChildren(...(selected !== null ? pickedStage() : idleStage(obs)));
  renderChoices();
  if (selected !== null) requestAnimationFrame(keepAboveDock);
  $("m-log").replaceChildren(...(obs.log || []).slice(-20).reverse().map((e) => el("li", e.type === "turn_started" ? "turn" : null, describeEvent(e))));
  $("m-dock").hidden = s.seat === null;
  if (s.seat !== null) renderDock(obs, byCard);
}

// Scroll the page so the stage and choice tiles sit fully above the fixed dock.
function keepAboveDock() {
  const list = $("m-choices");
  const last = list.hidden ? $("m-stage") : list;
  const overlap = last.getBoundingClientRect().bottom - $("m-dock").getBoundingClientRect().top;
  if (overlap > 0) window.scrollBy({ top: overlap + 8, behavior: "smooth" });
}

if (hostToken) {
  const b = el("button", "m-host", "New game");
  b.type = "button";
  b.onclick = () => $("new-game").click();
  $("m-conn").after(b);
}

// The dock is fixed over the page, so reserve its height at the bottom.
new ResizeObserver(([entry]) => {
  document.documentElement.style.setProperty("--m-dock-h", entry.target.offsetHeight + "px");
}).observe($("m-dock"));
