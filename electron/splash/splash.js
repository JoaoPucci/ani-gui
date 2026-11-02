// Starts the splash's motion. In Electron the main process adds `shown`
// itself on the window's `show` event (webContents.executeJavaScript), so
// the opening plays after the window has been mapped rather than during
// the 100-250 ms before it is. This timer is only the fallback for a
// missing call (and for previewing the page in a browser).
setTimeout(() => document.body.classList.add('shown'), 400);
