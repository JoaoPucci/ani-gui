const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { EventEmitter } = require("node:events");
const fc = require("fast-check");

const {
  SPLASH_MARK,
  SPLASH_PAGE,
  SPLASH_COMPLETE_HOLD_MS,
  SPLASH_COMPLETE_HOLD_REDUCED_MS,
  SPLASH_MIN_ON_SCREEN_MS,
  SPLASH_MIN_ON_SCREEN_REDUCED_MS,
  SPLASH_SHOW_WAIT_MS,
  lastSplashState,
  revealMaximized,
  splashController,
  splashPageQuery,
  splashVersionLabel,
  splashWindowOptions,
} = require("./splash.cjs");
const { STALL_MS, STEPS, stallTexts } = require("./splash-progress.cjs");
const { LOCALES, messagesFor } = require("./main-messages.cjs");

test("the splash is a small frameless window above everything, shown once painted", () => {
  const o = splashWindowOptions();
  assert.equal(o.frame, false);
  assert.equal(o.show, false);
  assert.equal(o.alwaysOnTop, true);
  assert.equal(o.resizable, false);
  assert.equal(o.skipTaskbar, false);
  assert.ok(o.width > 0 && o.width <= 640 && o.height > 0 && o.height <= 480);
  // The ink the app and the splash page paint (tokens.css --ink-000),
  // so nothing of another colour flashes before the page does.
  assert.equal(o.backgroundColor, require("./app-window.cjs").APP_BACKGROUND);
  assert.equal(o.webPreferences.contextIsolation, true);
  assert.equal(o.webPreferences.nodeIntegration, false);
  assert.equal(o.webPreferences.preload, undefined);
});

test("the splash page ships with the app", () => {
  assert.ok(fs.existsSync(SPLASH_PAGE), SPLASH_PAGE);
  const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf-8"));
  assert.ok(pkg.build.files.includes("splash/**/*"), "electron-builder's files allowlist carries it");
  assert.equal(SPLASH_MARK, "aniGuiSplash");
});

test("the splash page allows nothing but its own files", () => {
  const html = fs.readFileSync(SPLASH_PAGE, "utf-8");
  const csp = /http-equiv="Content-Security-Policy" content="([^"]*)"/.exec(html);
  assert.ok(csp, "the page carries a CSP");
  assert.equal(csp[1], "default-src 'self'");
});

test("the splash can be dragged out of the way", () => {
  const css = fs.readFileSync(path.join(path.dirname(SPLASH_PAGE), "splash.css"), "utf-8");
  assert.match(css, /-webkit-app-region:\s*drag/);
  assert.equal(splashWindowOptions().movable, true);
});

/** A fake BrowserWindow, enough for the controller. */
function fakeSplash() {
  const win = new EventEmitter();
  win.calls = [];
  win.destroyed = false;
  win.isDestroyed = () => win.destroyed;
  // As a real BrowserWindow does, show() emits "show" there and then.
  win.show = () => {
    win.calls.push("show");
    win.emit("show");
  };
  win.hide = () => win.calls.push("hide");
  win.close = () => {
    win.calls.push("close");
    win.destroyed = true;
    win.emit("closed");
  };
  win.loadFile = async (file, options) => {
    win.calls.push(`load:${path.basename(file)}`);
    win.loadOptions = options;
  };
  win.webContents = Object.assign(new EventEmitter(), {
    executeJavaScript: async (code) => {
      win.calls.push(`js:${code}`);
    },
  });
  return win;
}

test("a splash shows once it has painted, marked as the splash", () => {
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  assert.equal(win[SPLASH_MARK], true);
  assert.deepEqual(win.calls, ["load:index.html"]);
  win.emit("ready-to-show");
  const windowCalls = () => win.calls.filter((c) => !c.startsWith("js:"));
  assert.deepEqual(windowCalls(), ["load:index.html", "show"]);
  splash.close();
  assert.deepEqual(windowCalls(), ["load:index.html", "show", "close"]);
});

test("a splash suppressed before it painted never comes back on top of the failure report", () => {
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  splash.suppress();
  win.emit("ready-to-show");
  assert.ok(!win.calls.includes("show"), JSON.stringify(win.calls));
  assert.ok(win.calls.includes("hide"));
  // Hidden, not closed: closing the last window would start a quit.
  assert.ok(!win.calls.includes("close"));
});

test("closing or suppressing a splash that never opened, or is gone, does nothing", () => {
  const none = splashController({ createWindow: () => null, logError: () => {} });
  none.close();
  none.suppress();
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  win.destroyed = true;
  splash.close();
  splash.suppress();
  assert.deepEqual(win.calls, ["load:index.html"]);
});

test("a splash asked to show after the boot closed or suppressed it opens nothing", () => {
  // main.js opens the splash once the reduced-motion read settles, so a
  // boot can be over — window shown, or failed — before it asks.
  for (const end of ["close", "suppress"]) {
    let created = 0;
    const splash = splashController({
      createWindow: () => {
        created += 1;
        return fakeSplash();
      },
      logError: () => {},
    });
    splash[end]();
    splash.show();
    assert.equal(created, 0, end);
  }
});

test("a splash window that cannot be created is logged, and nothing else happens", () => {
  const logged = [];
  const splash = splashController({
    createWindow: () => {
      throw new Error("no display for a second window");
    },
    logError: (...args) => logged.push(args.join(" ")),
  });
  assert.doesNotThrow(() => splash.show());
  assert.equal(logged.length, 1);
  splash.close();
});

const splashDir = path.dirname(SPLASH_PAGE);
const read = (name) => fs.readFileSync(path.join(splashDir, name), "utf-8");

test("the splash is the pull-down: a film strip of the mark beside the title", () => {
  const html = read("index.html");
  assert.match(html, /class="track"/);
  assert.match(html, /class="wordmark"/);
  for (const file of ["splash.css", "splash.js", "version.js", "progress.js", "mark.svg", "sprocket.svg"]) {
    assert.ok(fs.existsSync(path.join(splashDir, file)), file);
  }
});

test("nothing on the splash counts: the slot under the title is a wordless rail", () => {
  // A ticking counter reads as a progress indicator, and so does any
  // number on a screen this brief. The slot under the title holds the
  // film-edge rail, three stretches and no words; the line under it is
  // empty until a step stalls (progress.js). This test pinned the slot
  // empty until the maintainer approved the rail for it.
  const html = read("index.html");
  assert.match(
    html,
    /<p class="readout"><span class="rail" aria-hidden="true"><i><\/i><i><\/i><i><\/i><\/span><span class="status" role="status"><\/span><\/p>\s*<script src="progress\.js"><\/script>/,
  );
  assert.doesNotMatch(html, /progress\.js"[^>]*\b(defer|async)\b/);
  assert.doesNotMatch(html, /class="(counter|still|unit)"/);
  const css = read("splash.css");
  assert.doesNotMatch(css, /counter\(|counter-reset|@counter-style|--pulled/);
  assert.doesNotMatch(css, /\.(counter|still|unit)\b/);
  // Nor anywhere else: the page's own words are its title alone, no
  // style writes text (the rail draws with an empty `content`), and the
  // scripts that write any write the app's version and the stall line.
  const body = /<body>([\s\S]*)<\/body>/.exec(html)[1];
  const words = body.replace(/<[^>]*>/g, " ").split(/\s+/).filter(Boolean);
  assert.deepEqual(words, ["ani-gui"]);
  assert.doesNotMatch(css, /\bcontent:(?!\s*""\s*;)|@property/);
  assert.doesNotMatch(read("splash.js"), /textContent|innerText|innerHTML|createTextNode|insertAdjacent/);
  assert.doesNotMatch(read("progress.js"), /innerHTML|insertAdjacent/);
});

test("the stall line sits outside the flow, so its coming and going moves nothing", () => {
  const css = read("splash.css");
  assert.match(css, /\.readout \{[^}]*position: relative;/);
  assert.match(css, /\.status \{[^}]*position: absolute;/);
  // A longer line, in any language, wraps inside the title column
  // rather than running off the window or being cut short (the
  // end-to-end suite checks every language's lines fit the window).
  assert.match(css, /\.status \{[^}]*max-inline-size: 100%;/);
  assert.doesNotMatch(css, /\.status \{[^}]*(nowrap|text-overflow)/);
});

test("the rail hangs under the title without pushing it off the gate's centre", () => {
  // The title and its rule centre on the gate, as they did with no rail:
  // the title column gives back the line the rail takes (the rail and
  // the row gap above it), so the stage centres the rest. A grid track
  // never goes below zero, so this cannot be the readout's own margin.
  const css = read("splash.css");
  const rail = /\.rail \{[^}]*block-size: (\d+)px;/.exec(css);
  const gap = /\.title \{[^}]*row-gap: (\d+)px;/.exec(css);
  assert.ok(rail && gap);
  assert.match(css, new RegExp(`\\.title \\{[^}]*margin-block-end: -${Number(rail[1]) + Number(gap[1])}px;`));
});

test("the rail and the stall line move only once the splash is told to move", () => {
  // Before the motion preference is known nothing may move: a step that
  // lands then shows lit at once. Under reduced motion, nothing moves
  // at all (the global still-frame rules, above).
  const css = read("splash.css");
  assert.doesNotMatch(css, /(^|\})\s*\.rail i::after \{[^}]*transition/);
  assert.doesNotMatch(css, /(^|\})\s*\.status \{[^}]*transition/);
  assert.match(css, /body\.shown \.rail i::after \{[^}]*transition: clip-path/);
  assert.match(css, /body\.shown \.status \{[^}]*transition: opacity/);
});

/** Run the splash's progress.js against a page loaded with `search`,
 *  on a fake clock. */
function progressPage(search) {
  const segments = [0, 1, 2].map(() => ({ classList: classes() }));
  const status = { textContent: "", classList: classes() };
  let now = 0;
  let timers = [];
  const context = {
    location: { search },
    URLSearchParams,
    Number,
    Math,
    setTimeout: (fn, ms) => {
      const t = { at: now + ms, fn };
      timers.push(t);
      return t;
    },
    clearTimeout: (t) => (timers = timers.filter((x) => x !== t)),
    document: {
      querySelectorAll: (sel) => (sel === ".rail i" ? segments : []),
      querySelector: (sel) => (sel === ".status" ? status : null),
    },
  };
  context.window = context;
  vm.runInNewContext(read("progress.js"), context);
  return {
    set: (n) => context.splashProgress.set(n),
    lit: () => segments.map((s) => s.classList.contains("lit")),
    status: () => (status.classList.contains("on") ? status.textContent : null),
    tick(ms) {
      const until = now + ms;
      for (;;) {
        const due = timers.filter((t) => t.at <= until).sort((a, b) => a.at - b.at)[0];
        if (!due) break;
        timers = timers.filter((t) => t !== due);
        now = due.at;
        due.fn();
      }
      now = until;
    },
  };
}
function classes() {
  const set = new Set();
  return { add: (c) => set.add(c), remove: (c) => set.delete(c), contains: (c) => set.has(c) };
}
const texts = "?st=700&t1=Backend%E2%80%A6&t2=Flags%E2%80%A6&t3=Page%E2%80%A6";

test("the rail lights a stretch per step landed, in order, and never goes back", () => {
  const page = progressPage(texts);
  assert.deepEqual(page.lit(), [false, false, false]);
  assert.equal(page.set(1), 1);
  assert.deepEqual(page.lit(), [true, false, false]);
  // Steps reported together light every stretch up to the last.
  assert.equal(page.set(3), 3);
  assert.deepEqual(page.lit(), [true, true, true]);
  assert.equal(page.set(2), 3);
  assert.deepEqual(page.lit(), [true, true, true]);
  assert.equal(page.set(9), 3);
});

test("the stall line comes only once a step has been pending 700 ms, and goes when it lands", () => {
  const page = progressPage(texts);
  page.tick(699);
  assert.equal(page.status(), null, "not before 700 ms");
  page.tick(1);
  assert.equal(page.status(), "Backend…");
  page.set(1);
  assert.equal(page.status(), null, "gone once the step lands");
  // The next step gets its own 700 ms, counted from the last landing.
  page.tick(699);
  assert.equal(page.status(), null);
  page.tick(1);
  assert.equal(page.status(), "Flags…");
  page.set(2);
  page.tick(300);
  page.set(3);
  page.tick(5000);
  assert.equal(page.status(), null, "nothing pending, nothing said");
});

test("a stall line that would say the same at the next step stays up instead of blinking", () => {
  const page = progressPage("?st=700&t1=Same&t2=Same&t3=Same");
  page.tick(700);
  assert.equal(page.status(), "Same");
  page.set(1);
  assert.equal(page.status(), "Same", "still pending, still the same words");
  page.tick(5000);
  assert.equal(page.status(), "Same");
  page.set(3);
  assert.equal(page.status(), null);
});

test("a page told nothing to say never shows a stall line", () => {
  const page = progressPage("");
  page.tick(5000);
  assert.equal(page.status(), null);
  assert.equal(page.set(1), 1);
});

test("a stalled flags step says the network is slow, and every other stalled step says the app is starting", () => {
  // The flags step waits on the network: the fetch of the flags file,
  // up to its two-second cap. Past 700 ms that is the cause we know of
  // (a slow read of the cached copy can hold it too). What the app
  // starts with is not yet decided when the line appears: the file, if
  // it arrives within the cap, else the saved copy or the defaults. So
  // the line promises only that the start goes ahead.
  assert.deepEqual(STEPS, ["backend", "flags", "page"]);
  assert.equal(STALL_MS, 700);
  const en = messagesFor("en");
  assert.equal(en.splashNetworkSlow, "Network is slow — starting in a moment…");
  assert.equal(en.splashStartingUp, "Starting up…");
  for (const locale of LOCALES) {
    const m = messagesFor(locale);
    for (const key of ["splashStartingUp", "splashNetworkSlow"]) {
      assert.equal(typeof m[key], "string", `${locale} ${key}`);
      assert.ok(m[key].length > 0, `${locale} ${key}`);
    }
    assert.notEqual(m.splashNetworkSlow, m.splashStartingUp, locale);
    assert.deepEqual(stallTexts(m), [m.splashStartingUp, m.splashNetworkSlow, m.splashStartingUp], locale);
  }
  for (const locale of LOCALES.filter((l) => l !== "en")) {
    assert.notEqual(messagesFor(locale).splashNetworkSlow, en.splashNetworkSlow, locale);
  }
});

test("on the page, the backend's stall says the generic line and the flags' stall the network line", () => {
  const m = messagesFor("en");
  const query = new URLSearchParams(splashPageQuery({ version: "0.18.0", stallTexts: stallTexts(m) }));
  const page = progressPage(`?${query}`);
  page.tick(STALL_MS);
  assert.equal(page.status(), m.splashStartingUp, "backend pending");
  page.set(1);
  assert.equal(page.status(), null, "the next step's line is other words");
  page.tick(STALL_MS - 1);
  assert.equal(page.status(), null);
  page.tick(1);
  assert.equal(page.status(), m.splashNetworkSlow, "flags pending");
  page.set(2);
  assert.equal(page.status(), null);
  page.tick(STALL_MS);
  assert.equal(page.status(), m.splashStartingUp, "page pending");
});

test("the splash page is opened with its version, its stall timing and its stall lines", () => {
  assert.deepEqual(splashPageQuery({ version: "0.18.0", stallTexts: ["a", "b", "c"] }), {
    v: "0.18.0",
    st: "700",
    t1: "a",
    t2: "b",
    t3: "c",
  });
  const win = fakeSplash();
  const splash = splashController({
    createWindow: () => win,
    logError: () => {},
    version: "0.18.0",
    stallTexts: () => ["a", "b", "c"],
  });
  splash.show();
  assert.deepEqual(win.loadOptions.query, { v: "0.18.0", st: "700", t1: "a", t2: "b", t3: "c" });
  splash.close();
});

/** A fake splash whose page answers each step with how many stretches
 *  it has lit. */
function railSplash() {
  const win = fakeSplash();
  let lit = 0;
  win.webContents.executeJavaScript = async (code) => {
    win.calls.push(`js:${code}`);
    const m = /splashProgress\.set\((\d+)\)/.exec(code);
    if (m) lit = Math.max(lit, Number(m[1]));
    return m ? lit : undefined;
  };
  return win;
}
const stepCalls = (win) => win.calls.filter((c) => c.includes("splashProgress"));

test("each boot step reaches the page as the count of steps landed, and the record keeps both", async () => {
  const win = railSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  await splash.progress("backend");
  await splash.progress("flags");
  // A step already passed, or one the splash does not know, changes nothing.
  await splash.progress("backend");
  await splash.progress("nonsense");
  await splash.progress("page");
  assert.deepEqual(stepCalls(win), [
    "js:window.splashProgress && window.splashProgress.set(1)",
    "js:window.splashProgress && window.splashProgress.set(2)",
    "js:window.splashProgress && window.splashProgress.set(3)",
  ]);
  assert.equal(lastSplashState().steps, 3);
  assert.equal(lastSplashState().railLit, 3);
  splash.close();
});

test("steps that landed before the page loaded reach it once it has", async () => {
  const win = railSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  await splash.progress("backend");
  await splash.progress("flags");
  const before = stepCalls(win).length;
  win.webContents.emit("did-finish-load");
  await tick();
  assert.equal(stepCalls(win).length, before + 1, "sent again on load");
  assert.equal(stepCalls(win).at(-1), "js:window.splashProgress && window.splashProgress.set(2)");
  splash.close();
});

test("a page that never answers a step holds nothing up for long", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = fakeSplash();
  win.webContents.executeJavaScript = () => new Promise(() => {});
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  let done = false;
  splash.progress("page").then(() => (done = true));
  await Promise.resolve();
  t.mock.timers.tick(149);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(done, false);
  t.mock.timers.tick(1);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(done, true);
  assert.equal(lastSplashState().steps, 3);
  assert.equal(lastSplashState().railLit, 0);
  splash.close();
});

test("a step with no splash to reach, or after it closed, is a no-op", async () => {
  const none = splashController({ createWindow: () => null, logError: () => {} });
  none.show();
  await none.progress("backend");
  const win = railSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  splash.close();
  await splash.progress("backend");
  assert.deepEqual(stepCalls(win), []);
});

const vm = require("node:vm");

/** Run the splash's version.js against a page loaded with `search`,
 *  returning what it leaves in the version line: its text, with an
 *  element written as `[class:text]`. */
function versionShown(search) {
  let children = [];
  const line = {
    append: (...nodes) => children.push(...nodes),
    set textContent(text) {
      children = [text];
    },
  };
  vm.runInNewContext(read("version.js"), {
    location: { search },
    URLSearchParams,
    document: {
      querySelector: (sel) => (sel === ".version" ? line : null),
      createElement: () => ({ className: "", textContent: "" }),
    },
  });
  return children.map((c) => (typeof c === "string" ? c : `[${c.className}:${c.textContent}]`)).join("");
}

test("the line above the title is the app's version, in the page before it paints", () => {
  const html = read("index.html");
  // Empty in the page itself, and filled by a script that runs as the
  // page is parsed — not deferred — so the first paint carries it.
  assert.match(html, /<p class="eyebrow version"><\/p>\s*<script src="version\.js"><\/script>/);
  assert.doesNotMatch(html, /version\.js"[^>]*\b(defer|async)\b/);
  // The leading "v" is set apart, in the app's accent; the version
  // itself, "-dev" included, is plain text.
  assert.equal(versionShown("?v=0.18.0"), "[v:v]0.18.0");
  assert.equal(versionShown("?v=0.18.0-dev"), "[v:v]0.18.0-dev");
  // A page opened without one (a browser preview) shows no line, and
  // the query is never taken as markup or as anything but a version.
  assert.equal(versionShown(""), "");
  assert.equal(versionShown("?v=%3Cimg%20src%3Dx%3E"), "");
  assert.equal(versionShown("?v=" + "9".repeat(80)), "");
  const css = read("splash.css");
  assert.match(css, /\.eyebrow:empty \{ display: none; \}/);
  // The eyebrow's capitals would turn "v0.18.0-dev" into "V0.18.0-DEV".
  assert.match(css, /\.version \{ text-transform: none; \}/);
  assert.match(css, /\.version \.v \{ color: var\(--brand\); \}/);
});

test("the splash's version is labelled as the app's rail and About label it", () => {
  // The rail and About show frontend/package.json's version, with
  // "-dev" in a dev run ($lib/version.ts); the splash reads Electron's,
  // which moves with it in every release, and marks a dev run the same.
  const ver = (p) => JSON.parse(fs.readFileSync(path.join(__dirname, "..", p), "utf-8")).version;
  assert.equal(ver("package.json"), ver(path.join("..", "frontend", "package.json")));
  for (const v of ["0.18.0", "1.2.3", "0.18.0-rc.1", ""]) {
    assert.equal(splashVersionLabel(v, false), v);
    assert.equal(splashVersionLabel(v, true), `${v}-dev`);
  }
  // Any version: shown as given, and marked only in a dev run.
  fc.assert(
    fc.property(fc.string(), fc.boolean(), (v, dev) => {
      assert.equal(splashVersionLabel(v, dev), dev ? `${v}-dev` : v);
    }),
  );
});

test("the splash is opened with the version it shows, and its record keeps it", () => {
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, version: "0.18.0" });
  splash.show();
  assert.equal(win.loadOptions.query.v, "0.18.0");
  assert.equal(lastSplashState().version, "0.18.0");
  splash.close();
});

test("the mark comes from mark.svg alone, so a new identity swaps one file", () => {
  const html = read("index.html");
  const srcs = [...html.matchAll(/<img [^>]*src="([^"]+)"/g)].map((m) => m[1]);
  assert.ok(srcs.length > 0);
  assert.deepEqual([...new Set(srcs)], ["mark.svg"]);
  assert.doesNotMatch(read("splash.css"), /mark\.svg/);
});

test("under reduced motion nothing on the splash moves", () => {
  assert.match(read("splash.css"), /@media \(prefers-reduced-motion: reduce\)[^{]*\{[^}]*animation: none !important/);
});

test("the splash holds still until it is told to move, and its fallback never starts motion", () => {
  // The splash opens before the app knows whether reduced motion is on,
  // so the page must not move on its own: its fallback, for a signal
  // that never comes, settles on the finished still frame.
  const css = read("splash.css");
  assert.match(css, /body:not\(\.shown\)[^{]*\{[^}]*animation-play-state: paused/);
  // The still frame applies to any page told to rest, so if the two
  // ever met, resting would win; the fallback itself (below) does not
  // rest a page the main process has set moving.
  assert.match(css, /body\.reduced \*[^{]*\{[^}]*animation: none !important/);
  assert.doesNotMatch(css, /body\.reduced:not\(\.shown\)/);
  const fallback =
    /^setTimeout\(\(\) => \{\n\s+if \(!document\.body\.classList\.contains\('shown'\)\) document\.body\.classList\.add\('reduced'\);\n\}, (\d+)\);$/m.exec(
      read("splash.js"),
    );
  assert.ok(fallback, "splash.js falls back to the still frame on a timer");
  assert.ok(Number(fallback[1]) <= 1000, `fallback after ${fallback[1]} ms`);
  assert.doesNotMatch(read("splash.js"), /classList\.add\('shown'\)/);
});

const tick = () => new Promise((r) => setImmediate(r));

/** A fake splash window whose shown signal answers when `answer` is
 *  called, recording the signal's code. */
function answeringSplash() {
  const win = fakeSplash();
  win.answer = () => {};
  win.webContents.executeJavaScript = (code) => {
    win.calls.push(`js:${code}`);
    return new Promise((resolve) => (win.answer = resolve));
  };
  return win;
}
const signals = (win) => win.calls.filter((c) => c.startsWith("js:"));

test("the splash opens at once, and is told to move only once the motion preference is known", async () => {
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  win.emit("ready-to-show");
  await tick();
  assert.deepEqual(signals(win), [], "no signal before the preference");
  splash.motionPreference(false);
  const [signal] = signals(win);
  assert.ok(signal.includes("classList.add('shown')"), signal);
  // It answers after two animation frames: a frame with the splash in
  // it has been rendered, which the window being mapped does not mean.
  assert.equal((signal.match(/requestAnimationFrame/g) || []).length, 2);
});

test("a preference known before the window shows is sent as it shows", () => {
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  splash.motionPreference(false);
  assert.deepEqual(signals(win), []);
  win.emit("ready-to-show");
  assert.equal(signals(win).length, 1);
});

test("under reduced motion the splash is told to rest on its still frame, never to move", () => {
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(true);
  const [signal] = signals(win);
  assert.ok(signal.includes("classList.add('reduced')"), signal);
  assert.ok(!signal.includes("classList.add('shown')"), signal);
});

test("the app's window waits until the splash has been on screen long enough for its opening to read", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  assert.equal(SPLASH_MIN_ON_SCREEN_MS, 500);
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(lastSplashState().shown, true);
  assert.equal(up, false, "painted, but not yet on screen long enough");
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS - 1);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(up, false);
  t.mock.timers.tick(1);
  await waiting;
  assert.equal(up, true);
});

test("a splash resting on its still frame needs a shorter time on screen", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  assert.equal(SPLASH_MIN_ON_SCREEN_REDUCED_MS, 300);
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  win.emit("ready-to-show");
  splash.motionPreference(true);
  // The page answers whether it is resting: here it is.
  win.answer(true);
  for (let i = 0; i < 5; i++) await tick();
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_REDUCED_MS - 1);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(up, false);
  t.mock.timers.tick(1);
  await waiting;
  assert.equal(up, true);
});

test("a splash that paints late still gets its time on screen, and the wait stays bounded", async (t) => {
  // The paint is waited for at most SPLASH_SHOW_WAIT_MS; a paint inside
  // that is followed by the minimum, so the wait never exceeds both.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  win.emit("ready-to-show");
  splash.motionPreference(false);
  t.mock.timers.tick(SPLASH_SHOW_WAIT_MS - 100);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  t.mock.timers.tick(100);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(up, false, "the bound on the paint does not cut the minimum short");
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS - 100);
  await waiting;
  assert.equal(up, true);
});

test("a splash that cannot be told it is shown keeps its fallback, is only logged, and holds nothing up", async () => {
  const win = fakeSplash();
  win.webContents.executeJavaScript = async () => {
    throw new Error("render frame was disposed");
  };
  const logged = [];
  const splash = splashController({ createWindow: () => win, logError: (...a) => logged.push(a) });
  splash.show();
  const waiting = splash.whenShown();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  await waiting;
  assert.equal(logged.length, 1);
});

test("a splash closed while the app's window waits for it releases the wait", async () => {
  const win = fakeSplash();
  win.webContents.executeJavaScript = () => new Promise(() => {});
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  const waiting = splash.whenShown();
  win.emit("ready-to-show");
  win.closeFromOutside = () => {
    win.destroyed = true;
    win.emit("closed");
  };
  win.closeFromOutside();
  await waiting;
});

test("what happened to the launch's splash can be read back afterwards", async () => {
  // A test run starts fast enough that the splash has come and gone
  // before a harness can look at its windows; the end-to-end suite
  // reads this instead.
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  assert.deepEqual(lastSplashState(), { opened: false, shown: false, closed: false, suppressed: false, version: "", steps: 0, railLit: 0 });
  splash.show();
  assert.deepEqual(lastSplashState(), { opened: true, shown: false, closed: false, suppressed: false, version: "", steps: 0, railLit: 0 });
  win.emit("ready-to-show");
  splash.motionPreference(false);
  await tick();
  splash.close();
  assert.deepEqual(lastSplashState(), { opened: true, shown: true, closed: true, suppressed: false, version: "", steps: 0, railLit: 0 });
  const failed = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  failed.show();
  failed.suppress();
  assert.deepEqual(lastSplashState(), { opened: true, shown: false, closed: false, suppressed: true, version: "", steps: 0, railLit: 0 });
});

test("the app's window can wait for the splash to be on screen, for a bounded time", async (t) => {
  // A fast start (a test run's, or a warm one) could otherwise show the
  // app's window before the splash had painted, and close a splash
  // nobody saw.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  await new Promise((r) => setImmediate(r));
  assert.equal(up, false);
  win.emit("ready-to-show");
  splash.motionPreference(false);
  for (let i = 0; i < 5; i++) await new Promise((r) => setImmediate(r));
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS);
  await waiting;
  assert.equal(up, true);

  const stuck = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  stuck.show();
  let gaveUp = false;
  const bounded = stuck.whenShown().then(() => (gaveUp = true));
  t.mock.timers.tick(SPLASH_SHOW_WAIT_MS - 1);
  await Promise.resolve();
  assert.equal(gaveUp, false);
  t.mock.timers.tick(1);
  await bounded;
  assert.equal(gaveUp, true);
});

test("there is nothing to wait for without a splash on its way", async () => {
  const none = splashController({ createWindow: () => null, logError: () => {} });
  none.show();
  await none.whenShown();
  const never = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  await never.whenShown();
  const gone = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  gone.show();
  gone.suppress();
  await gone.whenShown();
});

/** Settles `p` with no time allowed to pass: true if it settled. */
async function settlesAtOnce(t, p) {
  let done = false;
  p.then(() => (done = true));
  for (let i = 0; i < 5; i++) await tick();
  return done;
}

test("every way the splash cannot reach the screen releases the wait at once, not after its bound", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  // Closed while the signal is out (real disposal never answers it).
  const closing = fakeSplash();
  closing.webContents.executeJavaScript = () => new Promise(() => {});
  const a = splashController({ createWindow: () => closing, logError: () => {} });
  a.show();
  const aWait = a.whenShown();
  closing.emit("ready-to-show");
  closing.destroyed = true;
  closing.emit("closed");
  assert.equal(await settlesAtOnce(t, aWait), true, "closed");
  // The signal fails.
  const failing = fakeSplash();
  failing.webContents.executeJavaScript = async () => {
    throw new Error("render frame was disposed");
  };
  const b = splashController({ createWindow: () => failing, logError: () => {} });
  b.show();
  const bWait = b.whenShown();
  failing.emit("ready-to-show");
  b.motionPreference(false);
  assert.equal(await settlesAtOnce(t, bWait), true, "signal failed");
  // The signal answers.
  const fine = fakeSplash();
  const c = splashController({ createWindow: () => fine, logError: () => {} });
  c.show();
  const cWait = c.whenShown();
  fine.emit("ready-to-show");
  c.motionPreference(false);
  // Painted: it then has its minimum time on screen to serve.
  assert.equal(await settlesAtOnce(t, cWait), false, "painted, serving its minimum");
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS);
  assert.equal(await settlesAtOnce(t, cWait), true, "painted, minimum served");
  // The window cannot be made, or none is.
  for (const createWindow of [
    () => {
      throw new Error("no display");
    },
    () => null,
  ]) {
    const d = splashController({ createWindow, logError: () => {} });
    d.show();
    assert.equal(await settlesAtOnce(t, d.whenShown()), true, "never made");
  }
  // Still waiting while the signal is out and nothing has happened.
  const pending = fakeSplash();
  pending.webContents.executeJavaScript = () => new Promise(() => {});
  const e = splashController({ createWindow: () => pending, logError: () => {} });
  e.show();
  const eWait = e.whenShown();
  pending.emit("ready-to-show");
  assert.equal(await settlesAtOnce(t, eWait), false, "still out");
});

/** A fake of the app's window: records what is done to it. */
function fakeAppWindow() {
  const win = { calls: [], destroyed: false };
  win.isDestroyed = () => win.destroyed;
  win.maximize = () => win.calls.push("maximize");
  win.show = () => win.calls.push("show");
  return win;
}

test("revealing the app's window maps it maximized, shows it, then puts the splash away", () => {
  // Maximizing a hidden window maps it already maximized — the
  // open-maximized pattern; the listeners that act from the map on are
  // attached after it, and the splash goes once the window is shown,
  // not when its page later finishes loading.
  const app = fakeAppWindow();
  revealMaximized(app, {
    onMapped: () => app.calls.push("listening"),
    afterShow: () => app.calls.push("splash-closed"),
  });
  assert.deepEqual(app.calls, ["maximize", "listening", "show", "splash-closed"]);
});

test("an app window gone before it is revealed is left alone", () => {
  const app = fakeAppWindow();
  app.destroyed = true;
  revealMaximized(app, {
    onMapped: () => app.calls.push("listening"),
    afterShow: () => app.calls.push("splash-closed"),
  });
  assert.deepEqual(app.calls, []);
});

test("a splash the user closes asks the app to quit, and the boot reveals nothing", () => {
  // Once the app's window exists (hidden, loading), closing the splash
  // from the taskbar closes no last window, so no quit would follow of
  // itself; the user asked to stop the launch.
  const win = fakeSplash();
  let quits = 0;
  const splash = splashController({
    createWindow: () => win,
    logError: () => {},
    onDismissed: () => (quits += 1),
  });
  splash.show();
  win.emit("ready-to-show");
  assert.equal(splash.dismissed(), false);
  win.destroyed = true;
  win.emit("closed");
  assert.equal(quits, 1);
  assert.equal(splash.dismissed(), true);
});

test("closing or putting away the splash is not the user dismissing it", () => {
  for (const end of ["close", "suppress"]) {
    const win = fakeSplash();
    let quits = 0;
    const splash = splashController({
      createWindow: () => win,
      logError: () => {},
      onDismissed: () => (quits += 1),
    });
    splash.show();
    splash[end]();
    if (end === "suppress") {
      win.destroyed = true;
      win.emit("closed");
    }
    assert.equal(quits, 0, end);
    assert.equal(splash.dismissed(), false, end);
  }
});

test("a launch cancelled by closing the splash reveals nothing", () => {
  const app = fakeAppWindow();
  revealMaximized(app, {
    cancelled: () => true,
    onMapped: () => app.calls.push("listening"),
    afterShow: () => app.calls.push("splash-closed"),
  });
  assert.deepEqual(app.calls, []);
});

test("a launch's log says when the splash was asked for, shown, painted and closed", async (t) => {
  let now = 1000;
  const lines = [];
  const win = fakeSplash();
  const splash = splashController({
    createWindow: () => win,
    logError: () => {},
    log: (line) => lines.push(line),
    clock: () => now,
  });
  splash.show();
  now = 1180;
  win.emit("ready-to-show");
  now = 1230;
  splash.motionPreference(false);
  await tick();
  now = 1600;
  splash.close();
  assert.deepEqual(lines, [
    "[main] splash asked for (+0 ms)",
    "[main] splash shown (+180 ms)",
    "[main] splash painted (+230 ms)",
    "[main] splash closed by the boot (+600 ms, on screen 370 ms after painting)",
  ]);
});

test("the log says how a splash ended when the boot did not close it", () => {
  for (const [end, expected] of [
    ["suppress", "[main] splash put away: the boot failed (+0 ms)"],
    ["dismiss", "[main] splash closed before the boot was done (by the user or a quit): the launch is cancelled (+0 ms)"],
  ]) {
    const lines = [];
    const win = fakeSplash();
    const splash = splashController({
      createWindow: () => win,
      logError: () => {},
      log: (line) => lines.push(line),
      clock: () => 0,
    });
    splash.show();
    if (end === "suppress") splash.suppress();
    else {
      win.destroyed = true;
      win.emit("closed");
    }
    assert.equal(lines.at(-1), expected, end);
  }
});

test("a splash that never opens says so", () => {
  const lines = [];
  const splash = splashController({
    createWindow: () => null,
    logError: () => {},
    log: (line) => lines.push(line),
    clock: () => 0,
  });
  splash.show();
  assert.deepEqual(lines, ["[main] splash asked for (+0 ms)", "[main] splash not opened: no window was made"]);
});

test("a splash whose window cannot be made says so in the log, not only as an error", () => {
  const lines = [];
  const splash = splashController({
    createWindow: () => {
      throw new Error("no display");
    },
    logError: () => {},
    log: (line) => lines.push(line),
    clock: () => 0,
  });
  splash.show();
  assert.deepEqual(lines, [
    "[main] splash asked for (+0 ms)",
    "[main] splash not opened: the window could not be made",
  ]);
});

test("a splash closed before it painted says it never painted", () => {
  let now = 0;
  const lines = [];
  const win = fakeSplash();
  const splash = splashController({
    createWindow: () => win,
    logError: () => {},
    log: (line) => lines.push(line),
    clock: () => now,
  });
  splash.show();
  now = 90;
  splash.close();
  assert.deepEqual(lines, [
    "[main] splash asked for (+0 ms)",
    "[main] splash closed by the boot (+90 ms, never painted)",
  ]);
});

test("a window ready long after the splash painted pays nothing for the minimum", async (t) => {
  // The minimum counts from the paint, not from when the window asks.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 0;
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => now });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  now += 2000;
  t.mock.timers.tick(2000);
  assert.equal(await settlesAtOnce(t, splash.whenShown()), true);
});

test("a window ready part-way through the minimum waits only the rest of it", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 0;
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => now });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  now += 300;
  t.mock.timers.tick(300);
  const waiting = splash.whenShown();
  assert.equal(await settlesAtOnce(t, waiting), false);
  now += SPLASH_MIN_ON_SCREEN_MS - 300;
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS - 300);
  assert.equal(await settlesAtOnce(t, waiting), true);
});

test("a splash put away during its minimum releases the wait at once", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  const waiting = splash.whenShown();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  splash.suppress();
  assert.equal(await settlesAtOnce(t, waiting), true);
});

test("the shorter minimum follows the page's answer, which also reflects the system's own setting", async (t) => {
  // Where Chromium itself reports reduced motion (Windows, say), the
  // main process may pass no preference while the page still rests.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  const waiting = splash.whenShown();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(true);
  for (let i = 0; i < 5; i++) await tick();
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_REDUCED_MS - 1);
  assert.equal(await settlesAtOnce(t, waiting), false);
  t.mock.timers.tick(1);
  assert.equal(await settlesAtOnce(t, waiting), true);
});

test("a clock stepping back after the paint never stretches the wait past the minimum", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 10_000;
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => now });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  now -= 60_000; // the system clock set back a minute
  const waiting = splash.whenShown();
  // Let the wait reach its timer before time moves.
  assert.equal(await settlesAtOnce(t, waiting), false);
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS);
  assert.equal(await settlesAtOnce(t, waiting), true);
});


/** A splash on a hand-set clock and mock timers, painted at 0, whose
 *  page answers each step at once. Returns the controller, the clock
 *  setter, and `settle(promise)` which reports whether it has settled. */
async function paintedSplash(t, { resting = false, answer = (n) => Promise.resolve(n) } = {}) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 0;
  const win = fakeSplash();
  let answerShown = () => {};
  win.webContents.executeJavaScript = (code) => {
    const m = /splashProgress\.set\((\d+)\)/.exec(code);
    if (m) return answer(Number(m[1]));
    return new Promise((resolve) => (answerShown = resolve));
  };
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => now });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(resting);
  answerShown(resting);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(lastSplashState().shown, true, "painted");
  const at = (ms) => (now = ms);
  const flush = async () => {
    for (let i = 0; i < 8; i++) await tick();
  };
  const watch = (promise) => {
    const seen = { done: false };
    promise.then(() => (seen.done = true));
    return seen;
  };
  return { splash, win, at, flush, watch, now: () => now };
}

test("a last step landing after the minimum holds the splash until its stretch has been seen", async (t) => {
  // A slow page renders after the splash's minimum: without a hold the
  // last stretch would light in the instant the splash closes, and the
  // rail would never be seen complete.
  // The wipe is 300 ms on an ease-out curve, 99% of the way at 250 ms:
  // the hold is that, not the whole curve (the review of this hold
  // measured what 300 ms cost warm launches).
  assert.equal(SPLASH_COMPLETE_HOLD_MS, 250);
  const { splash, at, flush, watch } = await paintedSplash(t);
  at(SPLASH_MIN_ON_SCREEN_MS + 400);
  const step = watch(splash.progress("page"));
  await flush();
  assert.equal(step.done, false, "held");
  t.mock.timers.tick(SPLASH_COMPLETE_HOLD_MS - 1);
  await flush();
  assert.equal(step.done, false);
  t.mock.timers.tick(1);
  await flush();
  assert.equal(step.done, true);
  splash.close();
});

test("a last step landing well inside the minimum adds no wait: the minimum already shows it", async (t) => {
  const { splash, at, flush, watch } = await paintedSplash(t);
  at(100);
  const step = watch(splash.progress("page"));
  await flush();
  assert.equal(step.done, true);
  splash.close();
});

test("a last step landing just before the minimum ends is held only past it, until its stretch has been seen", async (t) => {
  // Landing 50 ms before the minimum ends, the stretch would show for
  // 50 ms: the reveal waits until the hold has passed since the step.
  const { splash, at, flush, watch } = await paintedSplash(t);
  at(SPLASH_MIN_ON_SCREEN_MS - 50);
  const step = watch(splash.progress("page"));
  await flush();
  assert.equal(step.done, false);
  t.mock.timers.tick(SPLASH_COMPLETE_HOLD_MS - 1);
  await flush();
  assert.equal(step.done, false);
  t.mock.timers.tick(1);
  await flush();
  assert.equal(step.done, true);
  splash.close();
});

test("on the still frame the stretch lights at once, so its hold is shorter", async (t) => {
  assert.equal(SPLASH_COMPLETE_HOLD_REDUCED_MS, 150);
  const { splash, at, flush, watch } = await paintedSplash(t, { resting: true });
  at(SPLASH_MIN_ON_SCREEN_REDUCED_MS + 400);
  const step = watch(splash.progress("page"));
  await flush();
  t.mock.timers.tick(SPLASH_COMPLETE_HOLD_REDUCED_MS - 1);
  await flush();
  assert.equal(step.done, false);
  t.mock.timers.tick(1);
  await flush();
  assert.equal(step.done, true);
  splash.close();
});

test("the hold is only for the last step, and ends at once if the splash goes", async (t) => {
  const { splash, at, flush, watch } = await paintedSplash(t);
  at(SPLASH_MIN_ON_SCREEN_MS + 400);
  const first = watch(splash.progress("backend"));
  await flush();
  assert.equal(first.done, true, "an earlier step is not held");
  const last = watch(splash.progress("page"));
  await flush();
  assert.equal(last.done, false);
  splash.close();
  await flush();
  assert.equal(last.done, true, "nothing to hold for once the splash is gone");
});

test("a last step landing before the splash has painted adds no wait: the minimum after the paint shows it", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = railSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  let done = false;
  splash.progress("page").then(() => (done = true));
  for (let i = 0; i < 8; i++) await tick();
  assert.equal(done, true);
  splash.close();
});

test("the hold counts from when the step landed: a slow answer is part of it, not added to it", async (t) => {
  let release;
  const { splash, at, flush, watch } = await paintedSplash(t, {
    answer: (n) => (n === 3 ? new Promise((r) => (release = () => r(n))) : Promise.resolve(n)),
  });
  at(SPLASH_MIN_ON_SCREEN_MS + 400);
  const step = watch(splash.progress("page"));
  await flush();
  // The page takes 100 ms to answer.
  t.mock.timers.tick(100);
  at(SPLASH_MIN_ON_SCREEN_MS + 500);
  release();
  await flush();
  assert.equal(step.done, false);
  t.mock.timers.tick(SPLASH_COMPLETE_HOLD_MS - 100 - 1);
  await flush();
  assert.equal(step.done, false);
  t.mock.timers.tick(1);
  await flush();
  assert.equal(step.done, true);
  splash.close();
});

test("a clock set back never stretches the hold", async (t) => {
  let release;
  const { splash, at, flush, watch } = await paintedSplash(t, {
    answer: (n) => (n === 3 ? new Promise((r) => (release = () => r(n))) : Promise.resolve(n)),
  });
  at(SPLASH_MIN_ON_SCREEN_MS + 400);
  const step = watch(splash.progress("page"));
  await flush();
  at(0); // the wall clock jumps back an hour, say
  release();
  await flush();
  t.mock.timers.tick(SPLASH_COMPLETE_HOLD_MS);
  await flush();
  assert.equal(step.done, true);
  splash.close();
});

test("a splash that painted after the reveal stopped waiting for its paint still shows the rail complete", async (t) => {
  // The reveal waits for the paint at most SPLASH_SHOW_WAIT_MS, and a
  // paint after that gets no minimum: the hold is then all that keeps
  // the last stretch on screen.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 0;
  const win = fakeSplash();
  let answerShown = () => {};
  win.webContents.executeJavaScript = (code) => {
    const m = /splashProgress\.set\((\d+)\)/.exec(code);
    if (m) return Promise.resolve(Number(m[1]));
    return new Promise((resolve) => (answerShown = resolve));
  };
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => now });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  let shownDone = false;
  splash.whenShown().then(() => (shownDone = true));
  t.mock.timers.tick(SPLASH_SHOW_WAIT_MS);
  for (let i = 0; i < 8; i++) await tick();
  assert.equal(shownDone, true, "the reveal stopped waiting");
  now = SPLASH_SHOW_WAIT_MS + 50;
  answerShown(false);
  for (let i = 0; i < 8; i++) await tick();
  now = SPLASH_SHOW_WAIT_MS + 100;
  let done = false;
  splash.progress("page").then(() => (done = true));
  for (let i = 0; i < 8; i++) await tick();
  assert.equal(done, false, "held, though the minimum after the paint has not run out");
  t.mock.timers.tick(SPLASH_COMPLETE_HOLD_MS);
  for (let i = 0; i < 8; i++) await tick();
  assert.equal(done, true);
  splash.close();
});

test("no language's network line says it starts regardless", () => {
  for (const locale of LOCALES) {
    assert.doesNotMatch(messagesFor(locale).splashNetworkSlow, /anyway|mesmo assim|de todos modos|всё равно|все равно|продолжится/i, locale);
  }
});
