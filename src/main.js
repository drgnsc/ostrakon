import { chippedOutline, drawFractures, polygonCss, seedFrom } from "./stone.js";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const appWindow = window.__TAURI__.window.getCurrentWindow();

const $ = (id) => document.getElementById(id);
const root = document.documentElement;
const slab = $("slab");
const tablet = $("tablet");
const scroller = $("scroller");
const docEl = $("doc");

const MARKDOWN_LINK = /\.(md|markdown|mdown|mkd|mdx)([#?].*)?$/i;
const KEY_DOCUMENT = /^(prd|roadmap)\.md$/i;
const RECENT_LIMIT = 8;
const SCALE_MIN = 0.8;
const SCALE_MAX = 1.5;

const state = {
  current: null,
  backStack: [],
  archive: [],
  roots: null,
  scanning: false,
  scale: 1,
  sidebarHidden: false,
};

// Per-viewer conveniences only: everything here tolerates missing storage.
const store = {
  get(key, fallback) {
    try {
      const raw = localStorage.getItem(`ostrakon.${key}`);
      return raw === null ? fallback : JSON.parse(raw);
    } catch {
      return fallback;
    }
  },
  set(key, value) {
    try {
      localStorage.setItem(`ostrakon.${key}`, JSON.stringify(value));
    } catch {}
  },
};

/* ---------- Stone ---------- */

let reshapeQueued = false;
function queueReshape() {
  if (reshapeQueued) return;
  reshapeQueued = true;
  requestAnimationFrame(() => {
    reshapeQueued = false;
    reshape();
  });
}

function reshape() {
  const maximized = root.classList.contains("maximized");
  const slabWidth = slab.clientWidth;
  const slabHeight = slab.clientHeight;
  const slabOutline = maximized
    ? null
    : chippedOutline(slabWidth, slabHeight, seedFrom("slab"), { roughness: 1.3, chipDepth: 5, chipChance: 0.05, cornerBite: 11 });
  const slabClip = slabOutline ? polygonCss(slabOutline) : "none";
  slab.style.clipPath = slabClip;
  $("slab-shadow").style.clipPath = slabClip;
  drawFractures($("slab-cracks"), slabWidth, slabHeight, seedFrom("slab-cracks"), { density: 0.55, reach: 0.22 });

  const seed = seedFrom(state.current?.path ?? "welcome");
  const width = tablet.clientWidth;
  const height = tablet.clientHeight;
  const outline = chippedOutline(width, height, seed, { roughness: 2, chipDepth: 9, chipChance: 0.09, cornerBite: 16, bigBreak: 54 });
  const clip = polygonCss(outline);
  tablet.style.clipPath = clip;
  $("tablet-shadow").style.clipPath = clip;
  drawFractures($("cracks"), width, height, seed, { density: 1, reach: 0.34, rim: outline });
}

new ResizeObserver(queueReshape).observe(slab);
new ResizeObserver(queueReshape).observe(tablet);

/* ---------- Theme and text size ---------- */

function applyTheme() {
  const chosen = store.get("theme", null);
  const dark = chosen ? chosen === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
  root.dataset.theme = dark ? "dark" : "light";
}

function toggleTheme() {
  store.set("theme", root.dataset.theme === "dark" ? "light" : "dark");
  applyTheme();
}

function setScale(value) {
  state.scale = Math.min(SCALE_MAX, Math.max(SCALE_MIN, Math.round(value * 100) / 100));
  root.style.setProperty("--scale", state.scale);
  store.set("scale", state.scale);
}

matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applyTheme);

/* ---------- Opening documents ---------- */

function toast(message, ms = 3200) {
  const el = $("toast");
  el.textContent = message;
  el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (el.hidden = true), ms);
}

async function open(command, args, { pushHistory = true, fragment = null } = {}) {
  let doc;
  try {
    doc = await invoke(command, args);
  } catch (error) {
    toast(String(error));
    return false;
  }
  if (!doc) return false;
  if (pushHistory && state.current && state.current.path !== doc.path) {
    state.backStack.push({ path: state.current.path, scroll: scroller.scrollTop });
  }
  showDocument(doc);
  rememberRecent(doc);
  if (fragment) {
    scrollToFragment(fragment);
  } else {
    scroller.scrollTop = 0;
  }
  scroller.focus({ preventScroll: true });
  return true;
}

const openPath = (path, options) => open("open_document", { path }, options);

function goBack() {
  const previous = state.backStack.pop();
  if (!previous) return;
  openPath(previous.path, { pushHistory: false }).then((ok) => {
    if (ok) scroller.scrollTop = previous.scroll;
  });
}

async function reloadCurrent(announce) {
  if (!state.current) return;
  const scroll = scroller.scrollTop;
  try {
    showDocument(await invoke("open_document", { path: state.current.path }));
    scroller.scrollTop = scroll;
    if (announce) toast("Odczytano ponownie", 1400);
  } catch (error) {
    toast(String(error));
  }
}

function showDocument(doc) {
  const changedArtifact = state.current?.path !== doc.path;
  state.current = doc;
  root.classList.add("has-doc");

  docEl.className = "";
  // Sanitized by ammonia on the Rust side; the CSP blocks scripts if that ever failed.
  docEl.innerHTML = doc.html;
  if (doc.meta.length) docEl.prepend(buildMeta(doc.meta));
  polishContent();

  renderCrumbs(doc);
  renderToc(doc.toc);
  renderSiblings(doc.siblings);
  renderStats(doc);
  observeHeadings();
  if (changedArtifact) reshape();
}

function buildMeta(pairs) {
  const list = document.createElement("dl");
  list.className = "meta";
  for (const [key, value] of pairs) {
    const term = document.createElement("dt");
    term.textContent = key.replaceAll("_", " ");
    const detail = document.createElement("dd");
    detail.textContent = value;
    list.append(term, detail);
  }
  return list;
}

function polishContent() {
  for (const img of docEl.querySelectorAll("img")) {
    if (img.hasAttribute("src")) {
      img.addEventListener("error", () => replaceImage(img, "brak obrazu"), { once: true });
    } else {
      replaceImage(img, "obraz zdalny zablokowany");
    }
  }
  for (const link of docEl.querySelectorAll("a[href]")) {
    if (!link.title && !link.getAttribute("href").startsWith("#")) link.title = link.getAttribute("href");
  }
}

function replaceImage(img, reason) {
  const note = document.createElement("span");
  note.className = "missing-image";
  note.textContent = img.alt ? `${reason}: ${img.alt}` : reason;
  img.replaceWith(note);
}

function renderCrumbs(doc) {
  const crumbs = $("crumbs");
  const place = document.createElement("span");
  place.className = "place";
  place.textContent = doc.crumbs.join(" / ");
  const name = document.createElement("span");
  name.className = "name";
  name.textContent = doc.name;
  crumbs.replaceChildren(place, name);
  crumbs.title = doc.path;
}

function renderToc(entries) {
  const visible = entries.filter((entry) => entry.level <= 3);
  $("toc-wrap").hidden = visible.length < 2;
  const baseLevel = Math.min(...visible.map((entry) => entry.level), 3);
  $("toc").replaceChildren(
    ...visible.map((entry) => {
      const item = document.createElement("li");
      item.className = `depth-${entry.level - baseLevel}`;
      const button = document.createElement("button");
      button.textContent = entry.text;
      button.dataset.target = entry.id;
      button.addEventListener("click", () => scrollToFragment(entry.id));
      item.append(button);
      return item;
    }),
  );
}

function renderSiblings(siblings) {
  $("siblings-wrap").hidden = siblings.length < 2;
  $("siblings").replaceChildren(
    ...siblings.map((sibling) => {
      const item = document.createElement("li");
      const button = document.createElement("button");
      button.textContent = sibling.name;
      if (sibling.current) button.setAttribute("aria-current", "true");
      button.addEventListener("click", () => openPath(sibling.path));
      item.append(button);
      return item;
    }),
  );
}

function renderStats(doc) {
  const minutes = Math.max(1, Math.round(doc.words / 200));
  const date = new Intl.DateTimeFormat("pl-PL", { dateStyle: "medium", timeStyle: "short" }).format(doc.modified);
  $("stats").textContent = `${doc.words.toLocaleString("pl-PL")} słów · ${minutes} min\n${date}`;
}

function scrollToFragment(fragment) {
  const target = document.getElementById(decodeURIComponent(fragment));
  if (target) scroller.scrollTop += target.getBoundingClientRect().top - scroller.getBoundingClientRect().top - 40;
}

let headingObserver = null;
function observeHeadings() {
  headingObserver?.disconnect();
  const buttons = new Map([...$("toc").querySelectorAll("button")].map((b) => [b.dataset.target, b]));
  if (!buttons.size) return;
  headingObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        for (const button of buttons.values()) button.removeAttribute("aria-current");
        const active = buttons.get(entry.target.id);
        active?.setAttribute("aria-current", "true");
        active?.scrollIntoView({ block: "nearest" });
      }
    },
    { root: scroller, rootMargin: "-6% 0px -82% 0px" },
  );
  for (const id of buttons.keys()) {
    const heading = document.getElementById(id);
    if (heading) headingObserver.observe(heading);
  }
}

/* ---------- Links ---------- */

docEl.addEventListener("click", (event) => {
  const link = event.target.closest("a[href]");
  if (!link) return;
  event.preventDefault();
  followLink(link.getAttribute("href"));
});
// Middle-click would otherwise try to spawn a new webview window.
docEl.addEventListener("auxclick", (event) => event.preventDefault());

function followLink(href) {
  if (href.startsWith("#")) {
    scrollToFragment(href.slice(1));
  } else if (/^(https?:|mailto:)/i.test(href)) {
    invoke("open_external", { url: href }).catch((error) => toast(String(error)));
  } else if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
    toast("Ten typ odnośnika jest zablokowany.");
  } else if (MARKDOWN_LINK.test(href)) {
    open("open_link", { href }, { fragment: href.split("#")[1] ?? null });
  } else {
    toast("Ostrakon otwiera tylko pliki Markdown.");
  }
}

/* ---------- Recent files and the archive ---------- */

function rememberRecent(doc) {
  const entry = { name: doc.name, path: doc.path, place: doc.crumbs.join(" / ") };
  const recents = store.get("recent", []).filter((r) => r.path !== doc.path);
  store.set("recent", [entry, ...recents].slice(0, RECENT_LIMIT));
}

async function refreshArchive() {
  if (state.scanning) return;
  state.scanning = true;
  renderPaletteStatus();
  try {
    state.archive = await invoke("archive_scan");
  } catch (error) {
    toast(String(error));
  }
  state.scanning = false;
  renderPaletteStatus();
  if ($("palette").open) renderPalette();
  if (!state.current) renderWelcome();
}

const fold = (text) => text.normalize("NFD").replace(/\p{M}/gu, "").replace(/ł/g, "l").replace(/Ł/g, "L").toLowerCase();

function searchArchive(query) {
  const tokens = fold(query).split(/\s+/).filter(Boolean);
  const scored = [];
  for (const entry of state.archive) {
    const name = fold(entry.name);
    const haystack = `${fold(entry.place)} ${name}`;
    if (!tokens.every((token) => haystack.includes(token))) continue;
    let score = KEY_DOCUMENT.test(entry.name) ? 4 : 0;
    for (const token of tokens) {
      if (name.startsWith(token)) score += 12;
      else if (name.includes(token)) score += 8;
    }
    scored.push({ entry, score });
  }
  scored.sort((a, b) => b.score - a.score || b.entry.modified - a.entry.modified);
  return scored.slice(0, 60).map((s) => s.entry);
}

function keyDocuments(limit) {
  return state.archive
    .filter((entry) => KEY_DOCUMENT.test(entry.name))
    .sort((a, b) => b.modified - a.modified)
    .slice(0, limit);
}

function documentRow(entry, onChoose) {
  const button = document.createElement("button");
  button.className = "row";
  button.type = "button";
  const name = document.createElement("span");
  name.className = "row-name";
  name.textContent = entry.name;
  const place = document.createElement("span");
  place.className = "row-place";
  place.textContent = entry.place;
  button.append(name, place);
  button.addEventListener("click", () => onChoose(entry));
  return button;
}

/* ---------- Palette ---------- */

const palette = $("palette");
const paletteInput = $("palette-input");
const paletteList = $("palette-list");
let selected = 0;

function openPalette() {
  if (palette.open) {
    paletteInput.select();
    return;
  }
  paletteInput.value = "";
  renderPalette();
  renderRoots();
  palette.showModal();
  paletteInput.focus();
}

function chooseFromPalette(entry) {
  palette.close();
  openPath(entry.path);
}

function renderPalette() {
  const query = paletteInput.value.trim();
  let groups;
  if (query) {
    groups = [{ title: null, entries: searchArchive(query) }];
  } else {
    const recents = store.get("recent", []);
    const seen = new Set(recents.map((r) => r.path));
    groups = [
      { title: "Ostatnio", entries: recents },
      { title: "PRD i roadmapy", entries: keyDocuments(40).filter((e) => !seen.has(e.path)) },
    ];
  }

  const items = [];
  const noRoots = state.roots?.length === 0;
  if (noRoots && !query) items.push(paletteNote("Archiwum jest puste. Dodaj folder z dokumentami przyciskiem „Dodaj folder” poniżej."));
  for (const group of groups) {
    if (!group.entries.length) continue;
    if (group.title) {
      const heading = document.createElement("li");
      heading.className = "kicker";
      heading.textContent = group.title;
      items.push(heading);
    }
    for (const entry of group.entries) {
      const item = document.createElement("li");
      item.append(documentRow(entry, chooseFromPalette));
      items.push(item);
    }
  }
  if (!items.length) {
    items.push(paletteNote(
      noRoots ? "Archiwum jest puste. Dodaj folder przyciskiem „Dodaj folder” poniżej."
        : state.scanning ? "Przeszukuję archiwum…" : "Nic nie znaleziono.",
    ));
  }
  paletteList.replaceChildren(...items);
  selected = 0;
  markSelected();
}

function paletteNote(text) {
  const note = document.createElement("li");
  note.className = "empty";
  note.textContent = text;
  return note;
}

function markSelected() {
  const rows = paletteList.querySelectorAll(".row");
  rows.forEach((row, index) => row.classList.toggle("selected", index === selected));
  rows[selected]?.scrollIntoView({ block: "nearest" });
}

function renderPaletteStatus() {
  $("palette-status").textContent = state.scanning ? "Skanuję…" : `${state.archive.length.toLocaleString("pl-PL")} dokumentów`;
}

async function renderRoots() {
  const roots = await invoke("archive_roots");
  const emptinessChanged = (state.roots?.length === 0) !== (roots.length === 0);
  state.roots = roots;
  if (emptinessChanged && palette.open) renderPalette();
  $("palette-roots").replaceChildren(
    ...roots.map((path) => {
      const chip = document.createElement("button");
      chip.type = "button";
      chip.className = "root-chip";
      chip.textContent = path;
      chip.title = "Usuń folder z archiwum";
      chip.addEventListener("click", async () => {
        await invoke("remove_root", { path });
        renderRoots();
        refreshArchive();
      });
      return chip;
    }),
  );
}

paletteInput.addEventListener("input", renderPalette);
paletteInput.addEventListener("keydown", (event) => {
  const rows = paletteList.querySelectorAll(".row");
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    if (!rows.length) return;
    selected = (selected + (event.key === "ArrowDown" ? 1 : -1) + rows.length) % rows.length;
    markSelected();
  } else if (event.key === "Enter") {
    event.preventDefault();
    rows[selected]?.click();
  }
});
palette.addEventListener("click", (event) => {
  if (event.target === palette) palette.close();
});
$("add-root-btn").addEventListener("click", async () => {
  await invoke("add_root");
  renderRoots();
  refreshArchive();
});

/* ---------- Welcome ---------- */

function renderWelcome() {
  docEl.className = "welcome";
  const hero = document.createElement("header");
  const mark = document.createElement("img");
  mark.src = "assets/mark.png";
  mark.alt = "";
  mark.width = 96;
  mark.height = 96;
  const title = document.createElement("h1");
  title.textContent = "Ostrakon";
  const line = document.createElement("p");
  line.className = "kicker";
  line.textContent = "Odczyt artefaktów z zamierzchłej przeszłości";
  hero.append(mark, title, line);

  const keys = document.createElement("p");
  keys.className = "keys";
  for (const [key, label] of [["Ctrl P", "archiwum"], ["Ctrl O", "otwórz plik"], ["upuść", "plik .md tutaj"]]) {
    const pair = document.createElement("span");
    const kbd = document.createElement("kbd");
    kbd.textContent = key;
    pair.append(kbd, ` ${label}`);
    keys.append(pair);
  }

  const parts = [hero, keys];
  const recents = store.get("recent", []).slice(0, 5);
  const seen = new Set(recents.map((r) => r.path));
  const groups = [
    ["Ostatnio", recents],
    ["PRD i roadmapy", keyDocuments(12).filter((e) => !seen.has(e.path)).slice(0, 7)],
  ];
  for (const [title, entries] of groups) {
    if (!entries.length) continue;
    const section = document.createElement("section");
    const heading = document.createElement("h2");
    heading.className = "kicker";
    heading.textContent = title;
    section.append(heading, ...entries.map((entry) => documentRow(entry, (e) => openPath(e.path))));
    parts.push(section);
  }
  docEl.replaceChildren(...parts);
}

/* ---------- Window chrome and keys ---------- */

$("archive-btn").addEventListener("click", openPalette);
$("theme-btn").addEventListener("click", toggleTheme);
$("min-btn").addEventListener("click", () => appWindow.minimize());
$("max-btn").addEventListener("click", () => appWindow.toggleMaximize());
$("close-btn").addEventListener("click", () => appWindow.close());

async function syncMaximized() {
  const maximized = await appWindow.isMaximized();
  if (maximized !== root.classList.contains("maximized")) {
    root.classList.toggle("maximized", maximized);
    queueReshape();
  }
}
appWindow.onResized(syncMaximized);

function toggleSidebar() {
  state.sidebarHidden = !state.sidebarHidden;
  root.classList.toggle("no-side", state.sidebarHidden);
  store.set("sidebarHidden", state.sidebarHidden);
}

const CTRL_KEYS = {
  o: () => open("pick_document", {}),
  p: openPalette,
  b: toggleSidebar,
  "\\": toggleSidebar,
  t: toggleTheme,
  r: () => reloadCurrent(true),
  w: () => appWindow.close(),
  "=": () => setScale(state.scale + 0.05),
  "+": () => setScale(state.scale + 0.05),
  "-": () => setScale(state.scale - 0.05),
  0: () => setScale(1),
};

window.addEventListener("keydown", (event) => {
  if (event.ctrlKey && !event.altKey && !event.shiftKey) {
    const action = CTRL_KEYS[event.key.toLowerCase()];
    if (action) {
      event.preventDefault();
      action();
    }
  } else if (event.key === "F5") {
    event.preventDefault();
    reloadCurrent(true);
  } else if (event.altKey && event.key === "ArrowLeft") {
    event.preventDefault();
    goBack();
  }
});
window.addEventListener("mouseup", (event) => {
  if (event.button === 3) goBack();
});
window.addEventListener("wheel", (event) => {
  if (event.ctrlKey) {
    event.preventDefault();
    setScale(state.scale + (event.deltaY < 0 ? 0.05 : -0.05));
  }
}, { passive: false });
// Keep the native menu only where it is useful: copying a selection.
window.addEventListener("contextmenu", (event) => {
  if (!String(getSelection())) event.preventDefault();
});

listen("open-path", (event) => openPath(event.payload));
listen("doc-changed", () => reloadCurrent(false));
listen("tauri://drag-enter", () => root.classList.add("dropping"));
listen("tauri://drag-leave", () => root.classList.remove("dropping"));
listen("tauri://drag-drop", (event) => {
  root.classList.remove("dropping");
  const path = event.payload.paths?.[0];
  if (path) openPath(path);
});

/* ---------- Start ---------- */

async function start() {
  applyTheme();
  setScale(store.get("scale", 1));
  state.sidebarHidden = store.get("sidebarHidden", false);
  root.classList.toggle("no-side", state.sidebarHidden);
  await syncMaximized();

  state.archive = await invoke("archive_cached");
  const pending = await invoke("initial_path");
  const opened = pending ? await openPath(pending) : false;
  if (!opened) renderWelcome();
  reshape();

  await document.fonts.ready;
  await invoke("show_window");
  refreshArchive();
}

start();
