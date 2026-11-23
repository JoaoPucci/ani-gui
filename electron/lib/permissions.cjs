"use strict";

// Which web permissions the window gets.
//
// Electron grants every permission request and check unless the session
// has a handler, so without this any document in the window could ask
// for notifications, the camera and microphone, location and the rest
// and be granted them. The app uses two:
//
//   fullscreen                  the player's fullscreen button
//   clipboard-sanitized-write   the About page's copy-address button
//
// Those two are granted to the app's own document (app-document.cjs)
// and nothing else is granted to anyone.

const { isAppDocument } = require("./app-document.cjs");

const ALLOWED_PERMISSIONS = new Set(["fullscreen", "clipboard-sanitized-write"]);

function permissionVerdict(permission, requestingUrl, app) {
  return ALLOWED_PERMISSIONS.has(permission) && isAppDocument(requestingUrl, app);
}

/** Install the verdict as `session`'s request and check handlers. */
function guardPermissions(session, { app }) {
  session.setPermissionRequestHandler((_contents, permission, callback, details) => {
    callback(permissionVerdict(permission, details && details.requestingUrl, app));
  });
  session.setPermissionCheckHandler((_contents, permission, requestingOrigin, details) => {
    const url = (details && details.requestingUrl) || requestingOrigin;
    return permissionVerdict(permission, url, app);
  });
}

module.exports = { ALLOWED_PERMISSIONS, guardPermissions, permissionVerdict };
