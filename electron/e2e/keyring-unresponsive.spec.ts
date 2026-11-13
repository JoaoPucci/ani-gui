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
import { expect, test } from "./policy-guard";
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import fs, { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { groupEmpty, statFields } from "../lib/own-group.cjs";
import { appInfo, defaultSettings, emptyHistory, topRated, trending } from "./fixtures/kitsu";
import {
  appWindow,
  launchPackagedApp,
  packagedBinary,
  refuseUnstubbed,
  remountFirstPage,
  routeContext,
  stopApp,
} from "./launch";

test.beforeAll(() => {
  if (!existsSync(packagedBinary)) {
    throw new Error(
      `prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`,
    );
  }
  // Chromium loads libsecret at run time to reach the secret service;
  // without it safeStorage falls back to basic_text at once, the bus is
  // never asked, and nothing here would be tested.
  const libraries = execFileSync("/sbin/ldconfig", ["-p"], {
    encoding: "utf8",
  });
  if (!libraries.includes("libsecret-1.so.0")) {
    throw new Error("prereq missing: libsecret-1.so.0\ninstall: libsecret-1-0");
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
    { stdio: ["ignore", "pipe", "pipe"], detached: true },
  );
  // The bus logs each activation with the pid of the connection that
  // asked for it: the process that is waiting on the keyring, as the
  // bus itself reports it.
  let log = "";
  daemon.stderr!.on("data", (chunk) => {
    log += chunk;
    process.stderr.write(chunk);
  });
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
    /** The pids that asked this bus for the secret service. */
    secretServiceRequesters: (): number[] =>
      [
        ...log.matchAll(
          /Activating service name='org\.freedesktop\.secrets' requested by '[^']*' \(uid=\d+ pid=(\d+)/g,
        ),
      ].map((m) => Number(m[1])),
    // Only while the daemon is unreaped: its group id is then still the
    // group this spec created (lib/own-group.cjs explains why).
    stop: () => {
      if (daemon.exitCode !== null || daemon.signalCode !== null) return;
      try {
        process.kill(-daemon.pid!, "SIGKILL");
      } catch {
        /* nothing left in the group */
      }
    },
  };
}

test("a secret service that never answers keeps neither the window nor the first page off screen", async () => {
  test.setTimeout(60_000);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ani-gui-e2e-keyring-"));
  // A saved sign-in: the read that asks the keyring.
  const tokens = path.join(tmp, "config", "ani-gui", "tokens");
  fs.mkdirSync(tokens, { recursive: true });
  const saved = Buffer.concat([Buffer.from("v11"), Buffer.alloc(32, 7)]);
  fs.writeFileSync(path.join(tokens, "anilist.bin"), saved);

  const bus = await busWithSilentSecretService(path.join(tmp, "bus")).catch((err) => {
    fs.rmSync(tmp, { recursive: true, force: true });
    throw err;
  });
  // The teardown below stops the bus and removes the profile once the
  // app is up; a bus or a launch that never starts is cleaned up here.
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
  }).catch((err) => {
    bus.stop();
    fs.rmSync(tmp, { recursive: true, force: true });
    throw err;
  });
  const pid = app.process().pid!;
  try {
    const context = app.context();
    const json = (body: unknown) => ({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(body),
    });
    // First, so it answers only what the stubs below leave; checked
    // empty at the end, for everything asked after the remount.
    const unstubbed = await refuseUnstubbed(context);
    let topRatedAsked: (() => void) | null = null;
    await routeContext(context, "**/api/app-info", (r) => r.fulfill(json(appInfo)));
    await routeContext(context, "**/api/history", (r) => r.fulfill(json(emptyHistory)));
    await routeContext(context, "**/api/kitsu/trending-anilist", (r) =>
      r.fulfill(json(trending)),
    );
    await routeContext(context, "**/api/kitsu/top-rated", (r) => {
      topRatedAsked?.();
      return r.fulfill(json(topRated));
    });
    // Every card known playable, so nothing is handed to the warm queue.
    await routeContext(context, "**/api/availability/batch", (r) => {
      const { kitsu_ids } = r.request().postDataJSON() as { kitsu_ids: string[] };
      return r.fulfill(
        json({
          cached: Object.fromEntries(kitsu_ids.map((id) => [id, true])),
          playable_episode_counts: {},
        }),
      );
    });
    await routeContext(context, "**/api/availability/warm", (r) => r.fulfill({ status: 202 }));
    await routeContext(context, "**/api/watched-at", (r) => r.fulfill(json({})));
    await routeContext(context, "**/api/update-check*", (r) => r.fulfill({ status: 204 }));
    await routeContext(context, "**/api/settings", (r) => r.fulfill(json(defaultSettings)));
    await routeContext(context, "**/api/image*", (r) => r.fulfill({ status: 503 }));

    // The app's window, not the splash that opens before it.
    const page = await appWindow(app);
    // Its first mount asked before the stubs were in place, and the
    // backend, kept off the network, could not answer: mount it again,
    // and go on once the new mount has asked a stub.
    await remountFirstPage(
      page,
      () => new Promise<void>((resolve) => (topRatedAsked = resolve)),
      { forget: unstubbed },
    );
    // The first page renders, signed out.
    await expect(page.getByText(/top rated/i).first()).toBeVisible({
      timeout: 15_000,
    });
    // The main process is free: it answers, and its window is up. The
    // splash closes once that window has shown, so the one window left
    // is the app's.
    await expect
      .poll(
        () =>
          app.evaluate(({ BrowserWindow }) =>
            BrowserWindow.getAllWindows().map((w) => w.isVisible()),
          ),
        { timeout: 10_000 },
      )
      .toEqual([true]);
    // The read is still out, and the app says what it is waiting on.
    await expect(page.locator(".toast .message")).toHaveText(
      [/Waiting for your system keyring/],
      { timeout: 10_000 },
    );
    // Nothing gave up on the saved sign-in.
    expect(fs.readFileSync(path.join(tokens, "anilist.bin"))).toEqual(saved);
    // The page ran on its stubs alone.
    expect(unstubbed, "requests no stub answered").toEqual([]);

    // One process asked the bus for the keyring: a helper of the app's,
    // not its main process, in the process group Playwright created for
    // the app.
    const asked = bus.secretServiceRequesters();
    expect(asked.length).toBe(1);
    const [helper] = asked;
    expect(helper).not.toBe(pid);
    const stat = statFields(fs.readFileSync(`/proc/${helper}/stat`, "utf8"));
    expect(stat?.pgrp).toBe(pid);

    // The helper still waiting on the keyring goes with the app: once the
    // app has quit, nothing is left in its group, the helper included.
    await app.close();
    await expect.poll(() => groupEmpty(pid), { timeout: 10_000 }).toBe(true);
  } finally {
    // Whatever failed above, the app's own group is stopped (nothing,
    // once it has exited); the backend stops itself with the app.
    await stopApp(app);
    bus.stop();
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});
