// A Board (#3073): notes, pictures, links and pen strokes on a canvas that pans and zooms. It reads
// its Board with GETs from chrome://board and writes it through chrome.send in slices, each
// answered before the next is sent (core/board.cc). Text is always set as text, never as markup,
// and nothing runs while the Board is idle: no timer outlives the edit or gesture that set it.

const SLICE = 256 * 1024;
// The most a Board's own file may be, as the host has it (core/board_store.h, kMaxBoardJson).
const MAX_BOARD = 4 * 1024 * 1024;
const tenth = (v) => Math.round(v * 10) / 10;
const MAX_FILE = 50 * 1024 * 1024;
const SMALL = 2048; // A picture draws from a copy at most this long on its long side.
const SAVE_AFTER = 1000;
const ZOOMS = [0.1, 4];
// tokens.css's swatches, by name: a note's fill, and with ink the pen's colours.
const SWATCHES = ["sand", "pink", "plum", "red", "orange", "yellow", "green", "blue", "slate"];
const FILLS = ["none", "plain", ...SWATCHES];
// A fill is its swatch over the plate, as much as the theme's share says.
const fillColor = (fill) => SWATCHES.includes(fill)
  ? `color-mix(in srgb, var(--fill-${fill}) var(--fill-share), var(--plate))` : "";

const id = location.pathname.slice(1);
const $ = (selector) => document.querySelector(selector);
const view = $("#view"), world = $("#world"), ink = $("#ink");

// --- The core's messages ---

// chrome.send's answers come back through cr, which a page of the browser's own defines.
const answers = new Map();
const listeners = new Map();
let asked = 0;
window.cr = {
  webUIResponse(callback, ok, value) {
    const answer = answers.get(callback);
    answers.delete(callback);
    answer?.(ok ? value : false);
  },
  webUIListenerCallback(event, ...args) {
    listeners.get(event)?.(...args);
  },
};
const send = (message, ...args) =>
  new Promise((answer) => {
    const callback = `board-${++asked}`;
    answers.set(callback, answer);
    chrome.send(message, [callback, ...args]);
  });

// Writes |bytes| as |kind| ("board", "thumb" or "file"), a slice at a time: the stored file's name
// for a picture, "" for the others, or false. The core takes one write at a time, so each waits for
// the one before: a thumbnail still being written as the tab shows again would fail a landing's save.
let writing = Promise.resolve();
function write(kind, bytes) {
  const now = writing.then(() => writeNow(kind, bytes));
  writing = now.catch(() => {});
  return now;
}
async function writeNow(kind, bytes) {
  if (!(await send("write", kind, bytes.byteLength))) return false;
  for (let at = 0; at < bytes.byteLength; at += SLICE) {
    // ArrayBuffer, which chrome.send carries as binary.
    if (!(await send("slice", bytes.slice(at, at + SLICE).buffer))) return false;
  }
  return send("done");
}

// --- The Board ---

let cards = [];
let camera = { x: 0, y: 0, zoom: 1 };
let writable = false;
const byId = new Map(); // card id => its element

const newId = () => crypto.randomUUID().replaceAll("-", "").slice(0, 16);
const cardOf = (cardId) => cards.find((card) => card.id === cardId);

// Only http(s) addresses become links, at paste and at Open.
function webAddress(text) {
  try {
    const url = new URL(text.trim());
    return url.protocol === "http:" || url.protocol === "https:" ? url.href : null;
  } catch {
    return null;
  }
}

// Undo and redo hold the cards as they were before each edit.
const undos = [], redos = [];
const snapshot = () => JSON.stringify(cards);
function edit(change) {
  undos.push(snapshot());
  if (undos.length > 200) undos.shift();
  redos.length = 0;
  change();
  changed();
}
function restore(from, to) {
  if (!from.length) return;
  to.push(snapshot());
  const focused = focusedCard();
  cards = JSON.parse(from.pop());
  selection.clear();
  renderAll();
  changed();
  // The cards were drawn again: the keyboard stays on them, on the card it was on if it's back.
  if (focused) cardOf(tabStop) ? focusCard(cardOf(tabStop)) : view.focus();
}

// --- Saving ---

let saveTimer = 0, saving = null, again = false, pending = false;
function changed() {
  if (!writable) return;
  pending = true;
  clearTimeout(saveTimer);
  saveTimer = setTimeout(save, SAVE_AFTER);
}
// Returns whether the Board as it is now was written.
async function save() {
  clearTimeout(saveTimer);
  if (!writable) return false;
  if (saving) {
    again = true;
    return saving.then((done) => done !== false);
  }
  pending = false;
  const bytes = new TextEncoder().encode(JSON.stringify({ version: 1, camera, cards }));
  if (bytes.byteLength > MAX_BOARD) {
    unsaved("This Board is too large to save. What's here stays on screen: remove some notes or ink, and it saves again.");
    return false;
  }
  saving = write("board", bytes);
  const done = await saving;
  saving = null;
  unsaved(done === false ? "This Board couldn't be saved. What's here stays on screen, and it tries again at your next change." : null);
  if (again) {
    again = false;
    return save();
  }
  return done !== false;
}

// Round the cards in the Library's picture, and the least it shows of the Board, in its own units:
// one note of the usual width fills half its square, a block that reads at the row's 18 points.
const PICTURE_MARGIN = 24, PICTURE_LEAST = 480;

// A card's height in the Board's own units: a note's card keeps 0, as its words size it as it shows.
function cardHeight(card) {
  return card.h || byId.get(card.id)?.offsetHeight || 0;
}

// The cards' bounds in the Board's own units, or null for an empty Board.
function cardsBounds() {
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
  const take = (x0, y0, x1, y1) => {
    left = Math.min(left, x0);
    top = Math.min(top, y0);
    right = Math.max(right, x1);
    bottom = Math.max(bottom, y1);
  };
  for (const card of cards) {
    if (card.kind === "stroke") {
      const r = card.width / 2;
      for (const [px, py] of card.points) take(card.x + px - r, card.y + py - r, card.x + px + r, card.y + py + r);
    } else {
      take(card.x, card.y, card.x + card.w, card.y + cardHeight(card));
    }
  }
  return left === Infinity ? null : { left, top, right, bottom };
}

// The Library's picture, 268 × 188 at twice that, of what's on the Board rather than what's in
// view: the cards' bounds, at least PICTURE_LEAST across, in its middle square, which is all of it
// the Library's row shows. The Library shows it on the paper of whichever look the window has, not
// the one it was taken in, so it has no ground of its own and draws only in colours that read on
// either: a note's fill as its swatch, and every other plate and the ink as --picture. A plate's
// words are left out, as they're too small to read there; a note with no plate is its words.
async function saveThumb() {
  const scale = 2, w = 268, h = 188;
  const canvas = new OffscreenCanvas(w * scale, h * scale);
  const g = canvas.getContext("2d");
  const style = getComputedStyle(document.documentElement);
  const swatch = (name) => style.getPropertyValue(SWATCHES.includes(name) ? `--fill-${name}` : "--picture").trim();
  const bounds = cardsBounds();
  if (bounds) {
    const across = Math.max(PICTURE_LEAST, bounds.right - bounds.left + 2 * PICTURE_MARGIN);
    const down = Math.max(PICTURE_LEAST, bounds.bottom - bounds.top + 2 * PICTURE_MARGIN);
    const fit = Math.min(h / across, h / down) * scale;
    g.translate((w * scale) / 2, (h * scale) / 2);
    g.scale(fit, fit);
    g.translate(-(bounds.left + bounds.right) / 2, -(bounds.top + bounds.bottom) / 2);
  }
  for (const card of cards) {
    const el = byId.get(card.id);
    if (card.kind === "stroke") {
      g.strokeStyle = swatch(card.color);
      g.lineWidth = card.width;
      g.lineCap = g.lineJoin = "round";
      g.stroke(new Path2D(strokePath(card)));
    } else if (card.kind === "picture" && el?.firstChild?.complete) {
      g.drawImage(el.firstChild, card.x, card.y, card.w, card.h);
    } else if (card.fill !== "none") {
      g.fillStyle = swatch(card.fill);
      g.beginPath();
      g.roundRect(card.x, card.y, card.w, cardHeight(card), 10);
      g.fill();
    } else {
      g.fillStyle = swatch();
      g.font = `${card.size === "l" ? 28 : 15}px system-ui`;
      g.fillText((card.text ?? "").split("\n")[0].slice(0, 60), card.x + 12, card.y + 28, card.w - 24);
    }
  }
  const png = new Uint8Array(await (await canvas.convertToBlob({ type: "image/png" })).arrayBuffer());
  if (writable) await write("thumb", png);
}

// --- Drawing the cards ---

const selection = new Set();

function place(el, card) {
  el.style.left = `${card.x}px`;
  el.style.top = `${card.y}px`;
  el.style.width = `${card.w}px`;
  if (card.kind !== "note") el.style.height = `${card.h}px`;
  el.style.zIndex = String(card.z ?? 0);
  el.classList.toggle("selected", selection.has(card.id));
}

function inkColor(color, style = getComputedStyle(document.documentElement)) {
  return style.getPropertyValue(SWATCHES.includes(color) ? `--fill-${color}` : "--ink").trim();
}
function strokePath(card) {
  const [first, ...rest] = card.points;
  if (!first) return "";
  return `M${card.x + first[0]} ${card.y + first[1]}` +
    rest.map(([px, py]) => ` L${card.x + px} ${card.y + py}`).join("");
}

function make(card) {
  let el;
  if (card.kind === "stroke") {
    el = document.createElementNS("http://www.w3.org/2000/svg", "path");
    el.setAttribute("d", strokePath(card));
    el.setAttribute("stroke", inkColor(card.color));
    el.setAttribute("stroke-width", String(card.width));
    el.dataset.id = card.id;
    name(el, card);
    ink.append(el);
    byId.set(card.id, el);
    return;
  }
  if (card.kind === "link") {
    el = document.createElement("a");
    const url = webAddress(card.url);
    if (url) el.href = url;
    el.target = "_blank";
    el.rel = "noopener noreferrer";
    const icon = document.createElement("img");
    icon.alt = "";
    if (url) icon.src = `chrome://favicon2/?size=32&pageUrl=${encodeURIComponent(url)}&allowGoogleServerFallback=0`;
    const text = document.createElement("span");
    text.textContent = card.title || url || card.url;
    const host = document.createElement("small");
    host.textContent = url ? new URL(url).host : "";
    text.append(host);
    el.append(icon, text);
  } else if (card.kind === "picture") {
    el = document.createElement("div");
    const img = document.createElement("img");
    img.alt = "";
    img.decoding = "async";
    img.draggable = false;
    img.src = `/${id}/files/${card.small || card.file}`;
    el.append(img);
  } else {
    el = document.createElement("div");
    const text = document.createElement("div");
    text.className = "text";
    text.spellcheck = true;
    text.textContent = card.text;
    el.append(text);
  }
  el.classList.add("card", card.kind);
  if (card.kind === "note") el.classList.add(card.size || "m", card.fill === "none" ? "none" : "filled");
  if (card.kind === "note" && fillColor(card.fill)) el.style.background = fillColor(card.fill);
  el.dataset.id = card.id;
  name(el, card);
  const grip = document.createElement("span");
  grip.className = "grip";
  el.append(grip);
  world.append(el);
  byId.set(card.id, el);
  place(el, card);
}

function renderAll() {
  for (const el of byId.values()) el.remove();
  byId.clear();
  if (!cardOf(tabStop)) tabStop = ordered()[0]?.id ?? null;
  for (const card of cards) make(card);
  showStyle();
}

// --- What a screen reader reaches ---

// A card's role and name, and whether Tab stops on it: a note by its first line, a picture as
// "Picture", as its file's name is only its bytes' hash, ink as "Drawing" and a link by its text.
function label(card) {
  if (card.kind === "picture") return "Picture";
  if (card.kind === "stroke") return "Drawing";
  if (card.kind === "link") return card.title || webAddress(card.url) || card.url;
  const line = card.text.split("\n").map((line) => line.trim()).find(Boolean);
  return line ? line.slice(0, 80) : "Empty note";
}
function name(el, card) {
  tabStop ??= card.id;
  if (card.kind === "note") el.setAttribute("role", "group"), el.setAttribute("aria-roledescription", "note");
  if (card.kind === "picture" || card.kind === "stroke") el.setAttribute("role", "img");
  if (card.kind !== "link") el.setAttribute("aria-label", label(card));
  el.tabIndex = card.id === tabStop ? 0 : -1;
}

// The cards are one stop of Tab's, the one the keyboard was last on or else the first in reading
// order, and Tab and ⇧Tab go on from card to card in that order: top to bottom, then left to right.
let tabStop = null;
const ordered = () => cards.slice().sort((a, b) => a.y - b.y || a.x - b.x);
function setTabStop(cardId) {
  if (tabStop === cardId) return;
  const was = byId.get(tabStop);
  if (was) was.tabIndex = -1;
  tabStop = cardId;
  const now = byId.get(cardId);
  if (now) now.tabIndex = 0;
}
function keepTabStop() {
  if (!byId.has(tabStop)) setTabStop(ordered()[0]?.id ?? null);
}
const inTools = (el) => Boolean(el?.closest?.("#tools, #style, #zoom, #note, #unsaved, #toast"));
const focusedCard = () => {
  const el = document.activeElement?.closest?.("[data-id]");
  return el && view.contains(el) ? cardOf(el.dataset.id) : null;
};
// Shows |card| with the keyboard on it, and pans to it if it's out of view.
function focusCard(card) {
  const el = byId.get(card.id);
  if (!el) return;
  setTabStop(card.id);
  el.focus({ preventScroll: true });
  const x = camera.x + card.x * camera.zoom, y = camera.y + card.y * camera.zoom;
  const w = card.w * camera.zoom, h = (card.h || el.offsetHeight) * camera.zoom;
  if (x < 0 || y < 0 || x + w > innerWidth || y + h > innerHeight) {
    camera.x = innerWidth / 2 - (card.x + card.w / 2) * camera.zoom;
    camera.y = innerHeight / 2 - (card.y + (card.h || el.offsetHeight) / 2) * camera.zoom;
    moveCamera();
  }
}
// The keyboard on a card selects it alone, as a click does. A click never moves the focus (the
// view's pointerdown keeps it), so this is Tab's, ⇧Tab's or the page's own.
world.addEventListener("focusin", (e) => {
  const el = e.target.closest?.("[data-id]");
  if (!el || editing) return;
  setTabStop(el.dataset.id);
  if (selection.size !== 1 || !selection.has(el.dataset.id)) {
    selection.clear();
    selection.add(el.dataset.id);
    renderSelection();
  }
});
// The view is the camera's, never scrolled: a focus that would scroll it pans instead.
view.addEventListener("scroll", () => (view.scrollTop = view.scrollLeft = 0));

// What a screen reader says of an edit the keyboard made: polite, unseen, and said again even when
// it's the same words, as each press of an arrow moves the cards again.
let said = false;
function announce(text) {
  said = !said;
  $("#said").textContent = said ? text : `${text}\u00a0`;
}
function redraw(card) {
  byId.get(card.id)?.remove();
  byId.delete(card.id);
  if (cardOf(card.id)) make(card);
}

// --- The camera ---

let settle = 0;
// |moved| is false for the camera the Board opened with, which needs no save.
function moveCamera(moved = true) {
  world.style.transform = `translate(${camera.x}px, ${camera.y}px) scale(${camera.zoom})`;
  $("#zoom-level").textContent = `${Math.round(camera.zoom * 100)}%`;
  // While it moves the world is one scaled layer; at rest it's drawn again, sharp, once.
  world.style.willChange = "transform";
  clearTimeout(settle);
  settle = setTimeout(() => {
    world.style.willChange = "";
    if (moved) changed();
    clearUnsaved();
  }, 200);
}
function zoomAt(zoom, cx = innerWidth / 2, cy = innerHeight / 2) {
  zoom = Math.min(ZOOMS[1], Math.max(ZOOMS[0], zoom));
  const k = zoom / camera.zoom;
  camera = { x: cx - (cx - camera.x) * k, y: cy - (cy - camera.y) * k, zoom };
  moveCamera();
}
const toWorld = (cx, cy) => [(cx - camera.x) / camera.zoom, (cy - camera.y) / camera.zoom];

view.addEventListener("wheel", (e) => {
  e.preventDefault();
  if (e.ctrlKey || e.metaKey) {
    zoomAt(camera.zoom * Math.exp(-e.deltaY * 0.01), e.clientX, e.clientY);
  } else {
    camera.x -= e.deltaX;
    camera.y -= e.deltaY;
    moveCamera();
  }
}, { passive: false });

// --- Tools ---

let tool = "select";
let style = { fill: "plain", size: "m", color: "ink", width: 4 };
function useTool(name) {
  tool = name;
  for (const button of document.querySelectorAll("[data-tool]")) {
    button.setAttribute("aria-pressed", String(button.dataset.tool === name));
  }
  view.className = name;
  showStyle();
}

// The style bar: a note's fill and size, or the pen's colour and width, for what's selected or
// the pen about to draw.
function showStyle() {
  const picked = [...selection].map(cardOf).filter(Boolean);
  const notes = picked.filter((card) => card.kind === "note");
  const strokes = picked.filter((card) => card.kind === "stroke");
  const pen = tool === "pen" || (strokes.length > 0 && notes.length === 0);
  $("#style").hidden = !pen && notes.length === 0;
  for (const part of ["#fills", "#sizes"]) $(part).hidden = pen;
  for (const part of ["#colors", "#widths"]) $(part).hidden = !pen;
  const now = {
    fill: notes[0]?.fill ?? style.fill, size: notes[0]?.size ?? style.size,
    color: strokes[0]?.color ?? style.color, width: String(strokes[0]?.width ?? style.width),
  };
  for (const key of ["fill", "size", "color", "width"]) {
    for (const b of document.querySelectorAll(`[data-${key}]`)) b.setAttribute("aria-checked", String(b.dataset[key] === now[key]));
  }
}
for (const color of ["ink", ...SWATCHES.slice(1)]) {
  const button = document.createElement("button");
  button.dataset.color = color;
  button.setAttribute("role", "radio");
  button.setAttribute("aria-label", color[0].toUpperCase() + color.slice(1));
  button.title = button.getAttribute("aria-label");
  button.style.background = color === "ink" ? "var(--ink)" : `var(--fill-${color})`;
  $("#colors").append(button);
}
for (const fill of FILLS) {
  const button = document.createElement("button");
  button.dataset.fill = fill;
  button.setAttribute("role", "radio");
  button.setAttribute("aria-label", fill === "none" ? "None" : fill[0].toUpperCase() + fill.slice(1));
  button.title = button.getAttribute("aria-label");
  if (fill === "none") button.className = "none";
  else button.style.background = fillColor(fill) || "var(--plate)";
  $("#fills").append(button);
}
$("#style").addEventListener("click", (e) => {
  const button = e.target.closest("button");
  if (!button) return;
  const key = ["fill", "size", "color", "width"].find((k) => button.dataset[k] !== undefined);
  const value = key === "width" ? Number(button.dataset.width) : button.dataset[key];
  style[key] = value;
  const kind = key === "fill" || key === "size" ? "note" : "stroke";
  const changing = [...selection].map(cardOf).filter((card) => card?.kind === kind);
  if (changing.length) edit(() => {
    for (const card of changing) {
      card[key] = value;
      redraw(card);
    }
  });
  showStyle();
});

$("#tools").addEventListener("click", (e) => {
  const button = e.target.closest("button");
  if (button?.dataset.tool) useTool(button.dataset.tool);
  if (button?.dataset.action === "picture") $("#chooser").click();
});
$("#zoom").addEventListener("click", (e) => {
  const action = e.target.closest("button")?.dataset.action;
  if (action === "zoom-in") zoomAt(camera.zoom * 1.25);
  if (action === "zoom-out") zoomAt(camera.zoom / 1.25);
});
$("#chooser").addEventListener("change", async (e) => {
  const [x, y] = toWorld(innerWidth / 2, innerHeight / 2);
  let at = 0;
  for (const file of e.target.files) await addPicture(file, x + at, y + at), at += 24;
  e.target.value = "";
});

// --- Adding ---

const top = () => cards.reduce((z, card) => Math.max(z, card.z ?? 0), 0) + 1;

function addNote(x, y, text = "") {
  const card = { id: newId(), kind: "note", x, y, w: 240, h: 0, z: top(), text, fill: style.fill, size: style.size };
  edit(() => cards.push(card));
  make(card);
  return card;
}

function addLink(url, x, y, title = "") {
  const card = { id: newId(), kind: "link", x, y, w: 280, h: 56, z: top(), url, title };
  edit(() => cards.push(card));
  make(card);
}

// A picture's bytes go to the core, which keeps them only if they're a PNG, JPEG, GIF or WebP. A
// big one also stores a small copy, which is what the Board draws.
async function addPicture(blob, x, y) {
  if (!writable || blob.size === 0 || blob.size > MAX_FILE) {
    if (blob.size > MAX_FILE) say("A picture can be at most 50 MB.", true);
    return;
  }
  const bytes = new Uint8Array(await blob.arrayBuffer());
  const file = await write("file", bytes);
  if (!file) return say("That isn't a picture a Board can keep.", true);
  let bitmap;
  try {
    bitmap = await createImageBitmap(blob);
  } catch {
    return say("That picture can't be read.", true);
  }
  const card = { ...(await picture(file, bitmap)), x, y };
  edit(() => cards.push(card));
  make(card);
}

// A picture card of the stored |file|, drawn from |bitmap|, at most 480 wide, with a small copy
// stored beside a big one; it's placed by the caller.
async function picture(file, bitmap) {
  let small = "";
  const long = Math.max(bitmap.width, bitmap.height);
  if (long > SMALL) {
    const k = SMALL / long;
    const canvas = new OffscreenCanvas(Math.round(bitmap.width * k), Math.round(bitmap.height * k));
    canvas.getContext("2d").drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    const copy = await canvas.convertToBlob({ type: "image/webp", quality: 0.9 });
    small = (await write("file", new Uint8Array(await copy.arrayBuffer()))) || "";
  }
  const w = Math.min(480, bitmap.width), h = Math.round((w * bitmap.height) / bitmap.width);
  bitmap.close();
  return { id: newId(), kind: "picture", x: 0, y: 0, w, h, z: top(), file, small };
}

// --- Arrivals ---

// A capture the browser put into this Board waits in its arrivals, a stored file's name a line
// (core/board.cc), until the page takes it. Each lands as a picture card at the centre of what's
// in view, a step down and right of any card already there so none hides another, and leaves the
// selection as a drop does. A landing is an edit, as a paste is, so ⌘Z takes it back. A Board whose
// tab isn't showing lands nothing until it's shown, so its arrivals land in the view the user sees.
// The arrivals are taken only once the save holding their cards is written: a crash between lands
// one again rather than losing it.
const STEP = 24, RUN = 8; // A run of 8 steps starts again a step across.
let landing = null, landAgain = false;
// Arrivals landed whose taking the core hasn't answered yet, which a landing passes over.
let held = [];
async function land() {
  if (!writable || document.visibilityState === "hidden") return;
  if (landing) {
    landAgain = true;
    return;
  }
  landing = landArrivals();
  await landing;
  landing = null;
  if (landAgain) {
    landAgain = false;
    return land();
  }
}
async function landArrivals() {
  const names = ((await read(`/${id}/arrived`)) || "").split("\n").filter(Boolean);
  if (!names.length) return;
  const fresh = names.slice();
  for (const name of held) {
    const at = fresh.indexOf(name);
    if (at >= 0) fresh.splice(at, 1);
  }
  held.push(...fresh);
  const landed = [];
  for (const file of fresh) {
    const blob = await read(`/${id}/files/${file}`, "blob");
    let bitmap;
    try {
      bitmap = blob && (await createImageBitmap(blob));
    } catch {
      bitmap = null;
    }
    // One that can't be read is taken all the same, as it can never land.
    if (bitmap) landed.push(await picture(file, bitmap));
  }
  if (landed.length) {
    edit(() => {
      for (const card of landed) {
        [card.x, card.y] = spot(card);
        card.z = top();
        cards.push(card);
      }
    });
    for (const card of landed) make(card);
  }
  // A save already under way doesn't hold the cards just landed.
  while (saving) await saving;
  if ((await save()) && (await send("taken", names))) held = [];
}
// Where |card| lands: centred in the view, or the first free place down a run of steps from there,
// each run a step across from the last, so a long list of arrivals stays in view.
function spot(card) {
  const [cx, cy] = toWorld(innerWidth / 2, innerHeight / 2);
  const step = STEP / camera.zoom;
  const x0 = cx - card.w / 2, y0 = cy - card.h / 2;
  const free = (x, y) => !cards.some((other) => other.kind !== "stroke" &&
    Math.abs(other.x - x) < step && Math.abs(other.y - y) < step);
  // Each place is rounded before it's tried, as a card is kept at whole pixels: a view an odd
  // number of pixels across would otherwise find a step from a card's place a half pixel short.
  for (let k = 0; k < RUN * RUN; k++) {
    const down = k % RUN;
    const x = Math.round(x0 + (down + Math.floor(k / RUN)) * step), y = Math.round(y0 + down * step);
    if (free(x, y)) return [x, y];
  }
  return [Math.round(x0), Math.round(y0)];
}

// What's pasted or dropped: a picture's bytes, a web address, or text as a note.
async function take(data, x, y) {
  const own = data.getData("application/x-ilo-board+json");
  if (own) return pasteCards(own, x, y);
  const files = [...data.files].filter((file) => file.type.startsWith("image/"));
  if (files.length) {
    for (const file of files) await addPicture(file, x, y), x += 24, y += 24;
    return;
  }
  const uri = (data.getData("text/uri-list") || "").split("\n").find((line) => line && !line.startsWith("#"));
  const text = data.getData("text/plain");
  const url = webAddress(uri || text || "");
  if (url) return addLink(url, x, y, uri && text !== uri ? text : "");
  // An address that isn't http(s) (javascript:, data:, file:, chrome:) is kept as text, never a link.
  if (text.trim()) addNote(x, y, text);
}

function pasteCards(json, x, y) {
  let copied;
  try {
    copied = JSON.parse(json);
  } catch {
    return;
  }
  if (!Array.isArray(copied) || !copied.length) return;
  const left = Math.min(...copied.map((card) => card.x)), up = Math.min(...copied.map((card) => card.y));
  selection.clear();
  edit(() => {
    for (const card of copied) {
      if (!["note", "picture", "link", "stroke"].includes(card.kind)) continue;
      if (card.kind === "link" && !webAddress(card.url)) continue;
      const pasted = { ...card, id: newId(), x: x + card.x - left, y: y + card.y - up, z: top() };
      cards.push(pasted);
      selection.add(pasted.id);
    }
  });
  renderAll();
}

// --- Pointer ---

let drag = null;
let spaceHeld = false;

view.addEventListener("pointerdown", (e) => {
  if (e.button === 1 || tool === "hand" || spaceHeld) {
    drag = { kind: "pan", sx: e.clientX, sy: e.clientY, cx: camera.x, cy: camera.y };
  } else if (tool === "pen") {
    const [x, y] = toWorld(e.clientX, e.clientY);
    const card = { id: newId(), kind: "stroke", x, y, w: 0, h: 0, z: top(), points: [[0, 0]], color: style.color, width: style.width };
    drag = { kind: "stroke", card, moved: false };
    cards.push(card);
    make(card);
  } else if (tool === "eraser") {
    drag = { kind: "erase" };
    erase(e);
  } else if (tool === "note") {
    const [x, y] = toWorld(e.clientX, e.clientY);
    const card = addNote(x - 20, y - 20);
    useTool("select");
    startEditing(card);
    e.preventDefault();
    return;
  } else {
    const el = e.target.closest(".card");
    if (el?.classList.contains("editing")) return;
    if (!el) {
      if (!e.shiftKey) selection.clear();
      stopEditing();
      renderSelection();
      return;
    }
    const cardId = el.dataset.id;
    if (e.shiftKey) selection.has(cardId) ? selection.delete(cardId) : selection.add(cardId);
    else if (!selection.has(cardId)) selection.clear(), selection.add(cardId);
    renderSelection();
    const grip = e.target.classList.contains("grip");
    const moving = [...selection].map(cardOf).filter(Boolean);
    drag = {
      kind: grip ? "resize" : "move", sx: e.clientX, sy: e.clientY, moved: false, before: snapshot(),
      from: moving.map((card) => ({ card, x: card.x, y: card.y, w: card.w, h: card.h })),
      link: el.classList.contains("link") ? el : null,
    };
  }
  view.setPointerCapture(e.pointerId);
  e.preventDefault();
});

view.addEventListener("pointermove", (e) => {
  if (!drag) return;
  clearUnsaved(true);
  const dx = (e.clientX - (drag.sx ?? 0)) / camera.zoom, dy = (e.clientY - (drag.sy ?? 0)) / camera.zoom;
  if (drag.kind === "pan") {
    camera.x = drag.cx + e.clientX - drag.sx;
    camera.y = drag.cy + e.clientY - drag.sy;
    moveCamera();
  } else if (drag.kind === "stroke") {
    const [x, y] = toWorld(e.clientX, e.clientY);
    // A tenth of a pixel is finer than a line can show, and keeps the Board's file small.
    drag.card.points.push([tenth(x - drag.card.x), tenth(y - drag.card.y)]);
    drag.moved = true;
    byId.get(drag.card.id).setAttribute("d", strokePath(drag.card));
  } else if (drag.kind === "erase") {
    erase(e);
  } else if (Math.abs(dx) + Math.abs(dy) > 2 / camera.zoom || drag.moved) {
    drag.moved = true;
    for (const from of drag.from) {
      if (drag.kind === "move") {
        from.card.x = from.x + dx;
        from.card.y = from.y + dy;
      } else if (from.card.kind !== "stroke") {
        from.card.w = Math.max(40, from.w + dx);
        if (from.card.kind === "picture") from.card.h = Math.round((from.card.w * from.h) / from.w);
        else if (from.card.kind === "link") from.card.h = Math.max(40, from.h + dy);
      }
      const el = byId.get(from.card.id);
      if (from.card.kind === "stroke") el.setAttribute("d", strokePath(from.card));
      else place(el, from.card);
    }
  }
});

view.addEventListener("pointerup", (e) => {
  const done = drag;
  drag = null;
  if (!done) return;
  if (done.kind === "stroke") {
    // A stroke's frame is its points' box, so it moves and copies as the others do.
    const xs = done.card.points.map((p) => p[0]), ys = done.card.points.map((p) => p[1]);
    done.card.w = Math.max(...xs) - Math.min(...xs);
    done.card.h = Math.max(...ys) - Math.min(...ys);
    const stroke = cards.pop();
    edit(() => cards.push(stroke));
  } else if ((done.kind === "move" || done.kind === "resize") && done.moved) {
    undos.push(done.before);
    redos.length = 0;
    changed();
  } else if (done.kind === "move" && !done.moved && done.link && !e.shiftKey) {
    // A click on a link opens it, http(s) only, as the user's own tab, with the click's modifiers.
    open(done.link, e);
  }
});

function open(link, e) {
  const url = webAddress(link.getAttribute("href") || "");
  if (!url) return;
  link.dispatchEvent(new MouseEvent("click", { bubbles: false, metaKey: e.metaKey, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, altKey: e.altKey }));
}
// The link's own click opens it only when it comes from open() above: a drag ends in a click too.
world.addEventListener("click", (e) => {
  if (e.target.closest("a") && e.isTrusted) e.preventDefault();
});

function erase(e) {
  const hit = document.elementFromPoint(e.clientX, e.clientY);
  const cardId = hit?.closest?.("#ink path")?.dataset.id;
  if (cardId) edit(() => {
    cards = cards.filter((card) => card.id !== cardId);
    byId.get(cardId)?.remove();
    byId.delete(cardId);
  });
  keepTabStop();
}

function renderSelection() {
  for (const card of cards) {
    const el = byId.get(card.id);
    if (card.kind !== "stroke") el?.classList.toggle("selected", selection.has(card.id));
    else el?.setAttribute("stroke-opacity", selection.has(card.id) ? "0.6" : "1");
  }
  showStyle();
  clearUnsaved();
}

// --- Notes ---

let editing = null;
function startEditing(card) {
  stopEditing();
  const el = byId.get(card.id);
  const text = el?.querySelector(".text");
  if (!text) return;
  editing = { card, before: snapshot(), text: card.text };
  el.classList.add("editing");
  text.contentEditable = "plaintext-only";
  text.focus();
  getSelection().selectAllChildren(text);
  getSelection().collapseToEnd();
  text.addEventListener("input", typed);
}
function typed(e) {
  if (editing) {
    editing.card.text = e.target.innerText;
    changed();
    clearUnsaved(true);
  }
}
function stopEditing() {
  if (!editing) return;
  const { card, before, text: was, first } = editing;
  // The first note's field is out of byId until the Board is open (offerFirstNote).
  const el = byId.get(card.id) ?? editing.el;
  editing = null;
  if (first) {
    // The first note's field, left before it took a key: it was never a card.
    el?.remove();
    byId.delete(card.id);
    return;
  }
  el?.classList.remove("editing");
  const text = el?.querySelector(".text");
  if (text) {
    text.contentEditable = "false";
    text.removeEventListener("input", typed);
  }
  if (!card.text.trim() && !was.trim()) {
    // An empty note left is gone, with no undo step of its own.
    cards = cards.filter((c) => c.id !== card.id);
    el?.remove();
    byId.delete(card.id);
    undos.pop();
    changed();
  } else if (card.text !== was) {
    undos.push(before);
    redos.length = 0;
    el?.setAttribute("aria-label", label(card));
  }
}
world.addEventListener("dblclick", (e) => {
  const el = e.target.closest(".note");
  if (el) startEditing(cardOf(el.dataset.id));
});
view.addEventListener("dblclick", (e) => {
  if (e.target.closest(".card") || tool !== "select") return;
  const [x, y] = toWorld(e.clientX, e.clientY);
  startEditing(addNote(x - 20, y - 20));
});

// On an empty Board the first note's field holds the keyboard unseen, so the Board takes the
// first keys, typed or composed (an IME's included), and the note appears with them. The field is
// the note's own, never moved to another parent, so a composition under way carries on in it.
// An input method composes only into a field, and held keys can't replay a composition, so the
// field is offered as the page starts, before the Board's file says whether it's empty (#3793):
// keepFirstNote, once the Board is open, keeps it or takes it away.
function offerFirstNote() {
  const card = { id: newId(), kind: "note", x: 0, y: 0, w: 240, h: 0, z: 1, text: "", fill: style.fill, size: style.size };
  make(card);
  const el = byId.get(card.id);
  // Out of the Board's cards until it's open, so drawing them (renderAll) leaves it be.
  byId.delete(card.id);
  // Not a card yet, so not Tab's stop: the page's first focus, as when it takes the keyboard
  // before it has loaded, would go to it rather than its field.
  if (tabStop === card.id) tabStop = null;
  el.tabIndex = -1;
  el.classList.add("first", "editing");
  centre(card, el);
  const field = el.querySelector(".text");
  field.contentEditable = "plaintext-only";
  const appear = () => {
    field.removeEventListener("beforeinput", appear);
    field.removeEventListener("compositionstart", appear);
    if (editing?.card !== card) return;
    el.classList.remove("first");
    if (opening) editing.taken = true;
    else becomeCard(card);
  };
  // The keys held while the Board opened make it appear too (opened, below).
  editing = { card, before: null, text: "", first: true, appear, el };
  field.addEventListener("beforeinput", appear);
  field.addEventListener("compositionstart", appear);
  field.addEventListener("input", typed);
  field.focus();
}
function becomeCard(card) {
  edit(() => cards.push(card));
  keepTabStop();
  editing = { card, before: undos.at(-1), text: "" };
}
// The first note's field at the centre of the view, as the camera is now.
function centre(card, el) {
  [card.x, card.y] = toWorld(innerWidth / 2 - 120, innerHeight / 2 - 40);
  place(el, card);
}
// Once the Board is open: an empty Board keeps the first note's field, and so does one with
// cards if something was typed or composed in it while it opened, as a note of its own. Returns
// whether the field stays.
function keepFirstNote() {
  if (!editing?.first) return false;
  const { card, el, taken } = editing;
  byId.set(card.id, el);
  if (!writable || (cards.length && !taken)) {
    stopEditing();
    return false;
  }
  card.z = top();
  centre(card, el);
  if (taken) becomeCard(card);
  return true;
}

// --- Keys ---

// The page has the keyboard as soon as it loads, but whether its Board is empty, and so whether
// it keeps the first note's field, waits for the host's hello and the Board's file. A key typed
// before then could be a note's letter or a tool's, so the keys that type are held until the Board
// is open (#3424): typed into the first note's field, the way typing there goes, or given to the
// canvas in order on a Board with cards. Tab and the arrows are held with them, in the order
// pressed, as the cards they go to and move aren't drawn yet (#3950). ⇧Tab isn't: from nothing
// focused it goes to the toolbar's last button, open or not.
let opening = true;
const early = [];
const walking = new Set(["Tab", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]);
document.addEventListener(
  "keydown",
  (e) => {
    // A key an input method takes (229) goes on to the first note's field, to compose there.
    if (!opening || e.isComposing || e.keyCode === 229) return;
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const walks = walking.has(e.key) && !(e.key === "Tab" && e.shiftKey);
    if (!walks && [...e.key].length !== 1) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    early.push({ key: e.key, shiftKey: e.shiftKey });
  },
  true,
);
function opened() {
  opening = false;
  const kept = keepFirstNote();
  const keys = early.splice(0);
  // A Board that didn't open has nothing for them to go to.
  if (!keys.length || !writable) return;
  if (kept) {
    // Only the keys that type: the first note's field has no card for Tab or an arrow.
    const text = keys.filter(({ key }) => !walking.has(key)).map(({ key }) => key).join("");
    if (!text) return;
    editing.appear?.();
    return document.execCommand("insertText", false, text);
  }
  for (const { key, shiftKey } of keys) {
    // From nothing focused, Tab goes to the first card in reading order, as the page's own Tab
    // does on an open Board. Past the last card a held Tab stays on it.
    if (key === "Tab" && !focusedCard() && cardOf(tabStop)) {
      focusCard(cardOf(tabStop));
      continue;
    }
    for (const type of ["keydown", "keyup"]) {
      document.body.dispatchEvent(new KeyboardEvent(type, { key, shiftKey, bubbles: true }));
    }
  }
}

document.addEventListener("keydown", (e) => {
  const command = e.metaKey || e.ctrlKey;
  // The canvas keeps ⌘+, ⌘− and ⌘0 for its own zoom, as Docs keeps ⌘F.
  if (command && (e.key === "=" || e.key === "+")) return e.preventDefault(), zoomAt(camera.zoom * 1.25);
  if (command && e.key === "-") return e.preventDefault(), zoomAt(camera.zoom / 1.25);
  if (command && e.key === "0") return e.preventDefault(), zoomAt(1);
  if (editing) {
    if (e.key === "Escape") {
      const card = editing.card;
      stopEditing();
      byId.has(card.id) ? focusCard(card) : view.focus();
    }
    return;
  }
  if (command && e.key.toLowerCase() === "z") {
    e.preventDefault();
    return e.shiftKey ? restore(redos, undos) : restore(undos, redos);
  }
  if (command && e.key.toLowerCase() === "a") {
    e.preventDefault();
    for (const card of cards) selection.add(card.id);
    renderSelection();
    return announce(cards.length === 1 ? "1 card selected" : `All ${cards.length} cards selected`);
  }
  if (e.key === "Delete" || e.key === "Backspace") {
    if (!selection.size) return;
    e.preventDefault();
    const gone = cards.filter((card) => selection.has(card.id));
    const order = ordered();
    const focused = focusedCard();
    const after = focused && order.slice(order.indexOf(focused)).find((card) => !selection.has(card.id));
    edit(() => {
      cards = cards.filter((card) => !selection.has(card.id));
      for (const cardId of selection) byId.get(cardId)?.remove(), byId.delete(cardId);
      selection.clear();
      showStyle();
    });
    keepTabStop();
    announce(gone.length === 1 ? `Deleted ${label(gone[0])}` : `Deleted ${gone.length} cards`);
    // The keyboard goes on to the card after, as a list's does once its item is gone.
    if (focused) {
      const next = after || ordered().findLast((card) => card.y < focused.y || (card.y === focused.y && card.x < focused.x));
      next ? focusCard(next) : view.focus();
    }
    return;
  }
  if (e.key === "Escape") {
    if (selection.size) announce("Nothing selected");
    return selection.clear(), renderSelection();
  }
  // The arrows move what's selected a point on screen, and with ⇧ ten, each press a step of ⌘Z's.
  const arrows = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  if (arrows[e.key] && selection.size && !command && !e.altKey && !inTools(e.target)) {
    e.preventDefault();
    const step = (e.shiftKey ? 10 : 1) / camera.zoom;
    const [dx, dy] = arrows[e.key].map((d) => d * step);
    edit(() => {
      for (const card of cards) {
        if (!selection.has(card.id)) continue;
        card.x += dx;
        card.y += dy;
        const el = byId.get(card.id);
        if (card.kind === "stroke") el?.setAttribute("d", strokePath(card));
        else if (el) place(el, card);
      }
    });
    return announce(`Moved ${e.key.slice(5).toLowerCase()}`);
  }
  // Tab and ⇧Tab go from card to card in reading order, and past the last or first out of them.
  if (e.key === "Tab" && !command && !e.altKey) {
    const focused = focusedCard();
    if (!focused) return;
    const order = ordered();
    const next = order[order.indexOf(focused) + (e.shiftKey ? -1 : 1)];
    if (next) e.preventDefault(), focusCard(next);
    return;
  }
  if (e.key === "Enter" && selection.size === 1) {
    const card = cardOf([...selection][0]);
    if (card?.kind === "note") return e.preventDefault(), startEditing(card);
    // A link card's own ↩ would be a click, which the Board keeps from opening it (below).
    if (card?.kind === "link") return e.preventDefault(), open(byId.get(card.id), e);
  }
  if (e.key === " ") spaceHeld = true, view.classList.add("panning");
  if (command || e.altKey) return;
  const tools = { v: "select", h: "hand", n: "note", p: "pen", e: "eraser" };
  if (tools[e.key]) useTool(tools[e.key]);
});
document.addEventListener("keyup", (e) => {
  if (e.key === " ") spaceHeld = false, view.classList.remove("panning");
});

// --- Clipboard and drops ---

function copied() {
  return cards.filter((card) => selection.has(card.id));
}
document.addEventListener("copy", (e) => {
  if (editing || !selection.size) return;
  const picked = copied();
  e.clipboardData.setData("application/x-ilo-board+json", JSON.stringify(picked));
  e.clipboardData.setData("text/plain", picked.map((card) => card.text ?? card.url ?? "").filter(Boolean).join("\n\n"));
  e.preventDefault();
});
document.addEventListener("cut", (e) => {
  if (editing || !selection.size) return;
  document.dispatchEvent(new ClipboardEvent("copy", { clipboardData: e.clipboardData }));
  const picked = copied();
  e.clipboardData.setData("application/x-ilo-board+json", JSON.stringify(picked));
  e.preventDefault();
  edit(() => {
    cards = cards.filter((card) => !selection.has(card.id));
    selection.clear();
  });
  renderAll();
});
document.addEventListener("paste", (e) => {
  const data = e.clipboardData;
  // Text pasted into the first note's field is its first input; anything else is a card.
  const card = data.files.length || data.getData("application/x-ilo-board+json") || webAddress(data.getData("text/plain"));
  if (editing && !(editing.first && card)) return;
  stopEditing();
  e.preventDefault();
  const [x, y] = toWorld(innerWidth / 2, innerHeight / 2);
  take(e.clipboardData, x, y);
});
view.addEventListener("dragover", (e) => e.preventDefault());
view.addEventListener("drop", (e) => {
  e.preventDefault();
  const [x, y] = toWorld(e.clientX, e.clientY);
  take(e.dataTransfer, x, y);
});

// --- Opening ---

// A save that didn't go through stays said over the Board until one does: what's on screen is
// kept, and nothing reads as saved that isn't.
// A save the core refused, said until one goes through: a note in the page's top corner, as a
// page's question hangs under the URL field beside it, never over the cards being worked on and
// never taking the keyboard.
function unsaved(text) {
  const note = $("#unsaved");
  note.textContent = text ?? "";
  note.hidden = !text;
  clearUnsaved();
}
// The top corner with fewer cards under the note, and never one with a card being worked on.
// While a card moves or grows (|moving|), only that is looked at, and the note moves away from it.
function clearUnsaved(moving = false) {
  const note = $("#unsaved");
  if (note.hidden) return;
  const worked = new Set([...selection, editing?.card.id]);
  const under = (aside) => {
    note.classList.toggle("aside", aside);
    const at = note.getBoundingClientRect();
    let count = 0;
    for (const cardId of moving ? worked : byId.keys()) {
      const r = byId.get(cardId)?.getBoundingClientRect();
      if (r && r.left < at.right && r.right > at.left && r.top < at.bottom && r.bottom > at.top) {
        if (worked.has(cardId)) return Infinity;
        count++;
      }
    }
    return count;
  };
  const aside = note.classList.contains("aside");
  if (moving) {
    if (under(aside) === Infinity) note.classList.toggle("aside", !aside);
    return;
  }
  const left = under(false), right = under(true);
  note.classList.toggle("aside", right < left);
}

// |text| in place of the Board, or for a moment over it.
let toastTimer = 0;
function say(text, briefly = false) {
  const note = $(briefly ? "#toast" : "#note");
  note.textContent = text;
  note.hidden = false;
  if (briefly) {
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (note.hidden = true), 2500);
  }
}

// |path| from the host as |type| ("text" or "blob"), or null. Blink's fetch() takes no chrome:
// address, so it's a GET the old way.
function read(path, type = "text") {
  return new Promise((done) => {
    const request = new XMLHttpRequest();
    request.open("GET", path);
    request.responseType = type;
    request.onload = () => done(request.status === 200 ? request.response : null);
    request.onerror = () => done(null);
    request.send();
  });
}

// Under the note of a Board that can't be read, its file in the system's file manager, to copy out
// or mend, as the page never writes over it (#3224). Chrome's word for that off the Mac. It takes
// the keyboard, as nothing else on the page does anything.
function offerFile() {
  const show = document.createElement("button");
  show.textContent = navigator.userAgent.includes("Mac OS X") ? "Show in Finder" : "Show in Folder";
  show.addEventListener("click", () => send("show"));
  $("#note").append(show);
  show.focus();
}

async function open_() {
  useTool("select");
  const answer = id ? await send("hello") : "missing";
  if (answer === "missing") return say("This Board isn't here.");
  if (answer === "elsewhere") {
    say("This Board is open in another tab.");
    listeners.set("board-free", () => location.reload());
    return;
  }
  // Every Board has its file from the start (app/src/boards.rs), so one that can't be read (over
  // the host's 4 MiB, or not parsing) is never written over.
  // Only an object is a Board: 5, [] or null parse, but would read as an empty one.
  const text = await read(`/${id}/board.json`);
  let board = null;
  try {
    board = JSON.parse(text);
  } catch {}
  if (!board || typeof board !== "object" || Array.isArray(board)) {
    say("This Board's file can't be read.");
    offerFile();
    return;
  }
  cards = Array.isArray(board.cards) ? board.cards : [];
  camera = { x: 0, y: 0, zoom: 1, ...board.camera };
  writable = true;
  $("#note").hidden = true;
  moveCamera(false);
  renderAll();
  // What came while it was closed lands now, and what comes while it's open lands as it comes.
  listeners.set("board-arrived", land);
  await land();
}

// Opening the bar or the Library takes the keyboard from the page, so what's done there with the
// Board, a Duplicate among it, finds its last change saved rather than a second old.
addEventListener("blur", () => { if (pending) save(); });
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "hidden" && writable) save().then(saveThumb);
  // What came while its tab wasn't showing lands now, at the centre of the view shown.
  else land();
});

offerFirstNote();
open_().finally(opened);
