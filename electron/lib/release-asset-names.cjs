"use strict";

// What electron-builder names each packaged file, next to the endings
// the in-app update check recognises (backend/src/meta/release_assets.json,
// embedded by backend/src/meta/github.rs). Build-time only: the test
// beside this file is its one caller.
//
// The names are electron-builder's own. A real LinuxPackager or
// WinPackager is built from this manifest's `build` config and asked
// through `expandArtifactNamePattern`, the method every target calls,
// so the arch spelling (`getArtifactArchName`: `x86_64` for an
// AppImage, `amd64` for a .deb, `x64` for the NSIS installer) and the
// macro expansion are whatever the installed electron-builder does.
// What is restated here is the extension each target hands that
// method (AppImageTarget "AppImage", FpmTarget its own target name,
// NsisTarget "exe" — the same in electron-builder 25 and 26) and the
// arch a target listed without one is built for: x64, the only arch
// the release runners build and the only one the update check
// recognises.
//
// A target whose name pattern is electron-builder's default, not one
// set in this manifest, is reported rather than expanded: whether a
// default pattern carries the arch depends on the target's own call
// (`skipDefaultArch`), which this file does not restate.

const fs = require("node:fs");
const path = require("node:path");

const ENDINGS_FILE = path.join(__dirname, "..", "..", "backend", "src", "meta", "release_assets.json");

/** The `build` key holding each platform's config, by the endings file's platform name. */
const PLATFORM_KEY = { linux: "linux", windows: "win" };

/** The `build` key holding a target's own options. */
const TARGET_OPTIONS = { AppImage: "appImage", nsis: "nsis" };

/** The extension a target passes to `expandArtifactNamePattern`. */
function extensionOf(target) {
  if (target === "AppImage") return "AppImage";
  if (target === "nsis") return "exe";
  return target;
}

function electronBuilder() {
  const entry = require.resolve("electron-builder", { paths: [path.join(__dirname, "..")] });
  return require(require.resolve("app-builder-lib", { paths: [entry] }));
}

function readEndings() {
  return JSON.parse(fs.readFileSync(ENDINGS_FILE, "utf8"));
}

/** `[{ target, archs }]` for one platform's `target` list. */
function targetsOf(platformConfig) {
  const list = Array.isArray(platformConfig.target) ? platformConfig.target : [platformConfig.target];
  return list.map((entry) =>
    typeof entry === "string"
      ? { target: entry, archs: ["x64"] }
      : { target: entry.target, archs: entry.arch ? [].concat(entry.arch) : ["x64"] },
  );
}

function packagerFor(lib, manifest, platform) {
  const info = { config: manifest.build, metadata: manifest, devMetadata: manifest, framework: {}, projectDir: path.join(__dirname, "..") };
  info.appInfo = new lib.AppInfo(info, null);
  return platform === "windows" ? new lib.WinPackager(info) : new lib.LinuxPackager(info);
}

/**
 * Every file the manifest packages: `{ platform, target, arch, name }`,
 * with `name` null and a `reason` when electron-builder's default
 * pattern would name it.
 */
function packagedArtifacts(manifest) {
  const lib = electronBuilder();
  const out = [];
  for (const [platform, key] of Object.entries(PLATFORM_KEY)) {
    const config = manifest.build[key];
    if (!config) continue;
    const packager = packagerFor(lib, manifest, platform);
    for (const { target, archs } of targetsOf(config)) {
      const options = manifest.build[TARGET_OPTIONS[target] ?? target];
      for (const arch of archs) {
        if (!packager.artifactPatternConfig(options).isUserForced) {
          out.push({ platform, target, arch, name: null, reason: "no artifactName set for it in electron/package.json" });
          continue;
        }
        const name = packager.expandArtifactNamePattern(options, extensionOf(target), lib.Arch[arch]);
        out.push({ platform, target, arch, name });
      }
    }
  }
  return out;
}

/**
 * The ways the manifest and the endings disagree: a packaged file
 * whose name does not end with its target's ending (or that has
 * none), and an ending no packaged target produces. Empty when they
 * agree.
 */
function mismatches(manifest, endings) {
  const found = [];
  const artifacts = packagedArtifacts(manifest);
  for (const a of artifacts) {
    const ending = endings[a.platform]?.[a.target];
    if (a.name == null) {
      found.push({ platform: a.platform, target: a.target, name: null, reason: a.reason });
    } else if (ending == null) {
      found.push({ platform: a.platform, target: a.target, name: a.name, reason: "the update check has no ending for this target" });
    } else if (!a.name.endsWith(ending)) {
      found.push({ platform: a.platform, target: a.target, name: a.name, reason: `does not end with ${ending}` });
    }
  }
  for (const [platform, targets] of Object.entries(endings)) {
    for (const target of Object.keys(targets)) {
      if (!artifacts.some((a) => a.platform === platform && a.target === target)) {
        found.push({ platform, target, name: null, reason: "no packaged target produces this ending" });
      }
    }
  }
  return found;
}

module.exports = { ENDINGS_FILE, packagedArtifacts, mismatches, readEndings };
