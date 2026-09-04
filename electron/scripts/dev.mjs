// Dev launcher: stages platform deps where needed, then starts
// Electron with the environment and flags `lib/dev-launch.cjs`
// computes. Exists so the `dev` package script contains no shell
// syntax — cmd.exe and POSIX shells both just run node.

import { spawn, spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { devEnv, devFlags, depsFetchScript } = require("../lib/dev-launch.cjs");

const here = path.dirname(fileURLToPath(import.meta.url));
const appDir = path.join(here, "..");
const platform = process.platform;

const fetcher = depsFetchScript(platform);
if (fetcher) {
  const staged = spawnSync(process.execPath, [path.join(here, fetcher)], {
    stdio: "inherit",
  });
  if (staged.status !== 0) process.exit(staged.status ?? 1);
}

// Resolving the `electron` package from a plain node process yields
// the path of the Electron binary.
const electron = require("electron");
const child = spawn(electron, [...devFlags(platform), "."], {
  cwd: appDir,
  env: { ...process.env, ...devEnv(platform, process.env) },
  stdio: "inherit",
});
child.on("exit", (code, signal) => process.exit(signal ? 1 : (code ?? 0)));
