// Says the page rendered 20 s after it loaded, the way the real
// frontend says it (two animation frames, then the preload's appReady;
// frontend/src/lib/app-ready.ts) — only late.
setTimeout(() => {
  requestAnimationFrame(() => requestAnimationFrame(() => window.aniGui.appReady()));
}, 20_000);
