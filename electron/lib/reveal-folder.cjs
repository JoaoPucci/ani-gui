"use strict";

// What revealInFolder will hand to shell.openPath.
//
// openPath opens its argument in the desktop's default handler: a
// folder opens in the file manager, but a file opens in its
// application, and an executable or a shortcut is run. The bridge's
// one caller, the download dock, passes a finished download's
// destination directory, so the handler takes an existing absolute
// directory and nothing else. That rules out files on Linux and
// Windows; on macOS, which runs from source only, an application
// bundle is a directory and openPath still launches it.
//
// `pathModule` and `statSync` are parameters so the Windows path rules
// can be tested on any host.

function isRevealableDirectory(p, { pathModule, statSync }) {
  if (typeof p !== "string" || p === "" || !pathModule.isAbsolute(p)) return false;
  try {
    return statSync(p).isDirectory();
  } catch {
    return false;
  }
}

module.exports = { isRevealableDirectory };
