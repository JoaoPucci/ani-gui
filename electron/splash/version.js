// The app's version, as the main process opened the page with it
// (`?v=`, lib/splash.cjs), written into the line above the title. The
// page loads this script where that line ends, without deferring it, so
// the version is there for the first paint. Anything that is not a
// version is ignored, and with none the line stays empty and hidden.
(() => {
  const v = new URLSearchParams(location.search).get('v');
  const line = document.querySelector('.version');
  if (!line || !v || !/^[0-9A-Za-z.+-]{1,40}$/.test(v)) return;
  // The leading "v" in its own element, in the app's accent
  // (splash.css); the version after it as plain text.
  const mark = document.createElement('span');
  mark.className = 'v';
  mark.textContent = 'v';
  line.append(mark, v);
})();
