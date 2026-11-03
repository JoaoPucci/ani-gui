/**
 * A secret service that never answers keeps neither the window nor
 * the first page off screen.
 *
 * Reading a saved sign-in asks the OS keyring to decrypt it, and
 * safeStorage's first call on Linux waits for the keyring's answer
 * however long it takes. Made from the main process, that froze the
 * whole app: a profile holding a saved sign-in, launched where the
 * secret service never answered, showed a blank frameless window that
 * never painted and ignored SIGTERM. The keyring helper
 * (lib/keyring-helper.cjs) now does the waiting.
 *
 * The launch gets a D-Bus session bus of its own whose
 * org.freedesktop.secrets is activatable but never arrives: the bus
 * starts the service's program, which never claims the name, and
 * holds every call to it while it waits — the shape of a secret
 * service that accepts a call and never replies. The page must render,
 * the main process must answer, the keyring notice must go up, and the
 * saved sign-in must be left as it was.
 */
import { expect, test } from "@playwright/test";
import { spawn, type ChildProcess } from "node:child_process";
import fs, { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { killTree } from "../lib/cold-launch.cjs";
import { appInfo, emptyHistory, topRated, trending } from "./fixtures/kitsu";
import { launchPackagedApp, packagedBinary } from "./launch";

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(
      `prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`,
    );
  }
});

/** A session bus whose secret service is activated and never shows up. */
async function busWithSilentSecretService(dir: string) {
  const services = path.join(dir, "services");
  fs.mkdirSync(services, { recursive: true });
  fs.writeFileSync(
    path.join(services, "org.freedesktop.secrets.service"),
    "[D-BUS Service]\nName=org.freedesktop.secrets\nExec=/bin/sleep 3600\n",
  );
  const config = path.join(dir, "bus.conf");
  fs.writeFileSync(
    config,
    `<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:dir=${dir}</listen>
  <servicedir>${services}</servicedir>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
  <limit name="service_start_timeout">3600000</limit>
</busconfig>
`,
  );
  // A process group of its own, so the bus and the program it
  // activated are stopped together.
  const daemon: ChildProcess = spawn(
    "dbus-daemon",
    [`--config-file=${config}`, "--nofork", "--print-address=1"],
    { stdio: ["ignore", "pipe", "inherit"], detached: true },
  );
  const address = await new Promise<string>((resolve, reject) => {
    let out = "";
    daemon.once("error", reject);
    daemon.stdout!.on("data", (chunk) => {
      out += chunk;
      const nl = out.indexOf("\n");
      if (nl >= 0) resolve(out.slice(0, nl).trim());
    });
  });
  return {
    address,
    stop: () => {
      try {
        process.kill(-daemon.pid!, "SIGKILL");
      } catch {
        /* already gone */
      }
    },
  };
}

/** The keyring helpers the app with main process `pid` started. */
function keyringHelpersOf(pid: number): number[] {
  const found: number[] = [];
  for (const entry of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(entry)) continue;
    try {
      const stat = fs.readFileSync(`/proc/${entry}/stat`, "utf8");
      const ppid = Number(stat.slice(stat.lastIndexOf(")") + 2).split(" ")[1]);
      const cmdline = fs.readFileSync(`/proc/${entry}/cmdline`, "utf8");
      // Its switch, or the process title it takes, which overwrites it.
      if (ppid === pid && cmdline.includes("keyring-helper")) {
        found.push(Number(entry));
      }
    } catch {
      // The process ended between the reads.
    }
  }
  return found;
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

test("a secret service that never answers keeps neither the window nor the first page off screen", async () => {
  test.setTimeout(60_000);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-e2e-keyring-"));
  // A saved sign-in: the read that asks the keyring.
  const tokens = path.join(tmp, "config", "ani-gui", "tokens");
  fs.mkdirSync(tokens, { recursive: true });
  const saved = Buffer.concat([Buffer.from("v11"), Buffer.alloc(32, 7)]);
  fs.writeFileSync(path.join(tokens, "anilist.bin"), saved);

  const bus = await busWithSilentSecretService(path.join(tmp, "bus"));
  const app = await launchPackagedApp({
    ...process.env,
    XDG_STATE_HOME: path.join(tmp, "state"),
    XDG_CONFIG_HOME: path.join(tmp, "config"),
    XDG_CACHE_HOME: path.join(tmp, "cache"),
    XDG_DATA_HOME: path.join(tmp, "data"),
    ANI_GUI_UNATTENDED: "1",
    DBUS_SESSION_BUS_ADDRESS: bus.address,
    // Chromium asks the secret service only on a desktop it recognises;
    // on any other (a CI runner has none) safeStorage falls back to
    // basic_text at once and never touches the bus. A GNOME desktop is
    // where the freeze was seen.
    XDG_CURRENT_DESKTOP: "GNOME",
  });
  const pid = app.process().pid;
  try {
    const context = app.context();
    const json = (body: unknown) => ({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(body),
    });
    await context.route("**/api/app-info", (r) => r.fulfill(json(appInfo)));
    await context.route("**/api/history", (r) => r.fulfill(json(emptyHistory)));
    await context.route("**/api/kitsu/trending", (r) =>
      r.fulfill(json(trending)),
    );
    await context.route("**/api/kitsu/top-rated", (r) =>
      r.fulfill(json(topRated)),
    );
    await context.route("**/api/image*", (r) => r.fulfill({ status: 503 }));

    const page = await app.firstWindow();
    // The first page renders, signed out.
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });
    // The main process is free: it answers, and its window is up.
    const visible = await app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((w) => w.isVisible()),
    );
    expect(visible).toEqual([true]);
    // The read is still out, and the app says what it is waiting on.
    await expect(page.locator(".toast .message")).toHaveText(
      [/Waiting for your system keyring/],
      { timeout: 10_000 },
    );
    // Nothing gave up on the saved sign-in.
    expect(fs.readFileSync(path.join(tokens, "anilist.bin"))).toEqual(saved);

    // The helper still waiting on the keyring goes with the app.
    const helpers = keyringHelpersOf(pid);
    expect(helpers.length).toBe(1);
    await app.close();
    await expect
      .poll(() => helpers.filter(alive), { timeout: 10_000 })
      .toEqual([]);
  } finally {
    // Whatever failed above, nothing is left behind: the tree kill
    // reaches the backend and the keyring helper.
    killTree(pid);
    bus.stop();
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});
