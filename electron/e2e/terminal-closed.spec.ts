/**
 * The app quits when it is told to, after the terminal it was started
 * from has closed, or after the reader of its output has gone.
 *
 * The main process writes its log to stdout and stderr: its own lines
 * and the backend's, relayed as [backend] lines. Once the terminal
 * closes, every one of those writes fails (EIO); once a pipe's reader
 * exits, the same (EPIPE). Unhandled, the first failure was an uncaught
 * exception in the main process, which opened Electron's "A JavaScript
 * error occurred" dialog in the middle of quitting: the window was
 * gone, the process never ended, and the profile stayed locked
 * (lib/stdio-guard.cjs).
 *
 * Each launch gets a scratch profile of its own, removed once the spec
 * has looked at it. The app is stopped the way a user stops it there:
 * closing the terminal hangs it up, which is the app's SIGHUP; on a
 * pipe, a SIGTERM to the app's own pid, as `kill` from another terminal
 * sends. It is signalled only while it is unreaped: on the pipe, it is
 * this spec's child, stopped with lib/own-group.cjs if it outlives the
 * wait, and what it leaves in its group is killed as its exit is
 * reported (spawnPackagedApp); on the terminal, it is the fixture's
 * child, and the fixture does the same.
 */
import { expect, test } from "./policy-guard";
import { execFileSync, type ChildProcess } from "node:child_process";
import fs, { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { stopOwnGroup } from "../lib/own-group.cjs";
import { packagedBinary, removeAfterExit, spawnPackagedApp } from "./launch";

/** What the main process logs once the app's first page is up: the
 *  boot is over, and the app has written to its stdout and stderr. */
const BOOTED = "[main] app's first page rendered";

/** How long a quit may take. Shutting down takes well under a second;
 *  the backend's own stop is not waited on (lib/backend-spawn.cjs). */
const QUIT_MS = 20_000;

const TERMINAL = path.join(__dirname, "fixtures", "terminal", "on-a-terminal.py");

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
  }
  // The terminal fixture is a Python script.
  try {
    execFileSync("python3", ["-c", "import pty"], { stdio: "ignore" });
  } catch {
    throw new Error("prereq missing: python3 (with its pty module), for e2e/fixtures/terminal/on-a-terminal.py");
  }
});

/** A scratch profile, and the environment that points the app at it. */
function scratchProfile(tag: string) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), `ani-gui-e2e-${tag}-`));
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    HOME: profile,
    XDG_CONFIG_HOME: path.join(profile, "config"),
    XDG_CACHE_HOME: path.join(profile, "cache"),
    XDG_STATE_HOME: path.join(profile, "state"),
    XDG_DATA_HOME: path.join(profile, "data"),
  };
  return { profile, env, locked: () => locked(profile) };
}

/** Whether Chromium's single-instance lock is in `profile`: there while
 *  the app runs, removed as it shuts down. Left behind, it means the
 *  app never shut down. The lock is a symlink to a name, not to a file,
 *  so it is looked at itself rather than followed. The profile itself
 *  must still be there, or a missing lock would prove nothing. */
function locked(profile: string): boolean {
  expect(existsSync(path.join(profile, "config", "ani-gui")), `profile ${profile} is gone`).toBe(true);
  try {
    fs.lstatSync(path.join(profile, "config", "ani-gui", "SingletonLock"));
    return true;
  } catch {
    return false;
  }
}

/** Resolves once `text()` contains `needle`, polling as output arrives. */
async function until(text: () => string, needle: string, timeoutMs: number): Promise<void> {
  await expect.poll(() => text().includes(needle), { timeout: timeoutMs }).toBe(true);
}

/** How `child` ended, or null if it is still running after `ms`. */
function ended(child: ChildProcess, ms: number): Promise<{ code: number | null; signal: string | null } | null> {
  if (child.exitCode !== null || child.signalCode !== null) {
    return Promise.resolve({ code: child.exitCode, signal: child.signalCode });
  }
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(null), ms);
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      resolve({ code, signal });
    });
  });
}

test("the app quits cleanly after the reader of its output has gone", async () => {
  test.setTimeout(90_000);
  const { profile, env, locked } = scratchProfile("reader-gone");
  // Resolved before the launch, so a launched app is always stopped.
  const exe = fs.realpathSync(`${packagedBinary}.bin`);
  let app: ChildProcess | undefined;
  try {
    app = await spawnPackagedApp(env, { stdio: ["ignore", "pipe", "pipe"] });
    let output = "";
    app.stdout!.on("data", (chunk) => (output += chunk));
    app.stderr!.on("data", (chunk) => (output += chunk));
    const running = app;
    await until(() => output, BOOTED, 60_000);
    expect(locked()).toBe(true);

    // The reader goes: every write the app makes from here fails.
    running.stdout!.destroy();
    running.stderr!.destroy();
    expect(running.exitCode === null && running.signalCode === null).toBe(true);
    process.kill(running.pid!, "SIGTERM");

    expect(await ended(running, QUIT_MS)).toEqual({ code: 0, signal: null });
    expect(locked()).toBe(false);
  } finally {
    try {
      if (app) await stopOwnGroup(app, { exe, termMs: 2000, killMs: 2000 });
    } finally {
      removeAfterExit(profile);
    }
  }
});

test("the app quits after the terminal it was started from has closed", async () => {
  test.setTimeout(90_000);
  const { profile, env, locked } = scratchProfile("terminal-closed");
  let terminal: ChildProcess | undefined;
  try {
    terminal = await spawnPackagedApp(env, {
      stdio: ["pipe", "pipe", "pipe"],
      via: ["python3", TERMINAL],
    });
    // A command written after the fixture has exited fails with EPIPE;
    // the fixture's own exit is what reports that.
    terminal.stdin!.on("error", () => {});
    let events = "";
    let output = "";
    terminal.stdout!.on("data", (chunk) => (events += chunk));
    terminal.stderr!.on("data", (chunk) => (output += chunk));
    await until(() => output, BOOTED, 60_000);
    expect(locked()).toBe(true);

    // Closing the terminal window: the app gets its SIGHUP, which asks
    // it to quit, and every write it makes to the terminal fails.
    terminal.stdin!.write("hangup\n");
    // The fixture's last complete line, once it is the app's ending
    // rather than its pid.
    const status = (): Record<string, number> | null => {
      const lines = events.split("\n").slice(0, -1);
      const last = lines.length > 0 ? JSON.parse(lines[lines.length - 1]) : null;
      return last && !("pid" in last) ? last : null;
    };
    await expect.poll(status, { timeout: QUIT_MS }).not.toBeNull();

    // The app shut down: the profile is unlocked.
    expect(locked()).toBe(false);
    // It ended without an error. Node puts a terminal it holds back as
    // it found it when the process exits, and on a terminal that has
    // hung up that step aborts (SIGABRT) after the app has shut down;
    // the launcher keeps the app's stdio off the terminal, relaying its
    // output, so there is nothing for that step to fail on
    // (lib/linux-launcher.cjs).
    expect(status()).toEqual({ exit: 0 });
  } finally {
    try {
      if (terminal) {
        // Kills the app's group if it is still running, then ends the
        // fixture, which kills what is left as its stdin closes.
        if (terminal.exitCode === null) terminal.stdin!.write("kill\n");
        terminal.stdin!.end();
        expect.soft(await ended(terminal, 10_000), "the terminal fixture did not end").not.toBeNull();
      }
    } finally {
      removeAfterExit(profile);
    }
  }
});
