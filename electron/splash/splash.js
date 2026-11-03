// Fallback for a signal that never comes (and for previewing the page
// in a browser). The page holds its first frame until the main process
// tells it, once the window is on screen and the desktop's reduced
// motion setting is known, either to start the motion (`shown`) or to
// rest on the finished still frame (`reduced`). The splash opens before
// that setting is read, so the page must never start motion on its own:
// this rests it instead.
setTimeout(() => document.body.classList.add('reduced'), 1000);
