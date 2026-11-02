"use strict";

// Which file of the packaged SvelteKit bundle an `app://` request names
// (the protocol handler in main.js).
//
// - A path that does not decode is a bad request. decodeURIComponent
//   throws on a malformed escape, and a throw out of the handler leaves
//   the request with no response at all — and so no policy header.
// - Anything that resolves outside the bundle directory is refused,
//   compared with path.relative rather than a string prefix, which a
//   sibling directory named like the bundle (`build-x`) would pass.
// - Otherwise the file, and whether the path is a route (no extension),
//   which the index answers when no such file exists.
//
// `pathModule` is node's `path` (or path.win32 / path.posix in tests):
// on Windows a decoded `%5C` is a separator and a drive letter makes a
// path absolute, and both are judged by that platform's rules.

function bundleTarget(bundleDir, rawPathname, pathModule) {
  let pathname;
  try {
    pathname = decodeURIComponent(rawPathname);
  } catch {
    return { kind: "bad" };
  }
  if (!pathname || pathname === "/") pathname = "/index.html";
  const file = pathModule.normalize(pathModule.join(bundleDir, pathname));
  const rel = pathModule.relative(bundleDir, file);
  if (rel === ".." || rel.startsWith(`..${pathModule.sep}`) || pathModule.isAbsolute(rel)) {
    return { kind: "forbidden" };
  }
  return { kind: "file", file, route: !pathModule.extname(pathname) };
}

module.exports = { bundleTarget };
