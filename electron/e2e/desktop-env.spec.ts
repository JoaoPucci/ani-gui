/**
 * What the packaged app starts on Linux starts with the user's desktop
 * environment, and the app itself keeps the one Electron gave it.
 *
 * Electron rewrites its own environment as it starts: on Ubuntu's
 * sessions, among others, XDG_CURRENT_DESKTOP becomes `Unity`, with the
 * session's value kept in ORIGINAL_XDG_CURRENT_DESKTOP, CHROME_DESKTOP names the app's
 * desktop file, and Chromium's GTK start-up adds GDK_BACKEND (and, under
 * X11, NO_AT_BRIDGE). A browser started from a link with all of that
 * believes it runs on Unity and puts its window buttons on the left.
 *
 * The app is launched as on Ubuntu's GNOME session (`ubuntu:GNOME`),
 * with stand-ins for xdg-open and busctl first on its PATH that record
 * the environment they were started with. It then opens a web page the
 * two ways the app's page can (the bridge's openExternal and a link
 * asking for a new window, which the window guard sends to the system
 * browser) and a folder (revealInFolder, shell.openPath), and starts
 * its reduced-motion read, whose first reader is busctl. Each recorded
 * environment must hold the session's desktop and none of what Electron
 * added; so must the backend's, which the external player and Syncplay
 * inherit. The main process's own environment must be the same after
 * every open as before it: the user's is in place only for the call.
 *
 * Pre-condition: `pnpm run package:e2e` has produced `dist-e2e/linux-unpacked/`.
 */
import { type ElectronApplication, type Page } from '@playwright/test';
import { expect, test } from './policy-guard';
import { withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { appWindow, launchPackagedApp, packagedBinary, stopApp, stopDeadApp } from './launch';

/** The session the launch pretends to be: Ubuntu's GNOME. */
const SESSION_DESKTOP = 'ubuntu:GNOME';
/** What Electron or Chromium put in the app's environment, which no child may get. */
const INJECTED = ['ORIGINAL_XDG_CURRENT_DESKTOP', 'CHROME_DESKTOP', 'GDK_BACKEND', 'NO_AT_BRIDGE'];

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

/** A stand-in for `name` in `bin` that records how it was started in
 *  `seen` and exits with `code`. The record appears whole or not at all. */
function recorder(bin: string, seen: string, name: string, code: number) {
	const script = [
		'#!/bin/sh',
		`out="${seen}/${name}-$$"`,
		`printf '%s' "$*" > "$out.args"`,
		`env -0 > "$out.tmp" && mv "$out.tmp" "$out.env"`,
		`exit ${code}`,
		''
	].join('\n');
	fs.writeFileSync(path.join(bin, name), script, { mode: 0o755 });
}

type Recorded = { args: string; env: NodeJS.Dict<string> };

/** Every record `name` has left in `seen`. */
function records(seen: string, name: string): Recorded[] {
	return fs
		.readdirSync(seen)
		.filter((f) => f.startsWith(`${name}-`) && f.endsWith('.env'))
		.map((f) => {
			const base = path.join(seen, f.slice(0, -'.env'.length));
			return {
				args: fs.readFileSync(`${base}.args`, 'utf-8'),
				env: parseEnv(fs.readFileSync(`${base}.env`, 'utf-8'))
			};
		});
}

/** A NUL-separated environment (`env -0`, /proc/<pid>/environ). */
function parseEnv(text: string): NodeJS.Dict<string> {
	const env: NodeJS.Dict<string> = {};
	for (const entry of text.split('\0')) {
		const at = entry.indexOf('=');
		if (at > 0) env[entry.slice(0, at)] = entry.slice(at + 1);
	}
	return env;
}

/** The user's desktop environment, and nothing Electron added. */
function expectUserDesktop(env: NodeJS.Dict<string>, what: string) {
	expect(env.XDG_CURRENT_DESKTOP, what).toBe(SESSION_DESKTOP);
	for (const name of INJECTED) expect(env[name], `${what}: ${name}`).toBeUndefined();
}

/** The backend: the child of the app's main process running its binary. */
function backendPid(main: number): number {
	const children = fs
		.readdirSync(`/proc/${main}/task`)
		.flatMap((tid) => {
			try {
				return fs.readFileSync(`/proc/${main}/task/${tid}/children`, 'utf-8').split(' ');
			} catch {
				return []; // a thread that ended in between
			}
		})
		.filter(Boolean)
		.map(Number);
	const backend = children.filter((pid) => {
		try {
			return path.basename(fs.readlinkSync(`/proc/${pid}/exe`)) === 'ani-gui-backend';
		} catch {
			return false;
		}
	});
	expect(backend, `children of ${main}: ${children.join(' ')}`).toHaveLength(1);
	return backend[0];
}

async function launchOnce(onLaunch: (app: ElectronApplication) => void) {
	const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-e2e-desktop-env-'));
	const bin = path.join(profile, 'bin');
	const seen = path.join(profile, 'seen');
	fs.mkdirSync(bin);
	fs.mkdirSync(seen);
	recorder(bin, seen, 'xdg-open', 0);
	// The reduced-motion read goes on to its next reader.
	recorder(bin, seen, 'busctl', 1);
	const env: NodeJS.ProcessEnv = { ...process.env };
	for (const name of INJECTED) delete env[name];
	Object.assign(env, {
		HOME: profile,
		XDG_CONFIG_HOME: path.join(profile, 'config'),
		XDG_CACHE_HOME: path.join(profile, 'cache'),
		XDG_STATE_HOME: path.join(profile, 'state'),
		XDG_DATA_HOME: path.join(profile, 'data'),
		XDG_CURRENT_DESKTOP: SESSION_DESKTOP,
		PATH: `${bin}:${process.env.PATH ?? '/usr/bin:/bin'}`
	});
	const app = await launchPackagedApp(env, { profile });
	onLaunch(app);
	const page = await appWindow(app);
	return { app, page, profile, seen };
}

async function launch(): Promise<{ app: ElectronApplication; page: Page; profile: string; seen: string }> {
	let pending: ElectronApplication | null = null;
	return withColdLaunchRetry(
		async () => {
			const launched = await launchOnce((app) => {
				pending = app;
			});
			pending = null;
			return launched;
		},
		{
			cleanup: async () => {
				const dead = pending;
				pending = null;
				if (dead) await stopDeadApp(dead);
			}
		}
	);
}

test("what the app opens starts with the user's desktop environment, and the app keeps its own", async () => {
	const { app, page, profile, seen } = await launch();
	try {
		const mainEnv = () => app.evaluate(() => ({ ...process.env }));
		const before = await mainEnv();
		// The app's own environment is Electron's: the condition this proves
		// anything about.
		expect(before.XDG_CURRENT_DESKTOP).toBe('Unity');
		expect(before.ORIGINAL_XDG_CURRENT_DESKTOP).toBe(SESSION_DESKTOP);
		expect(before.CHROME_DESKTOP).toBeTruthy();

		await page.evaluate(() => window.aniGui.openExternal('https://example.invalid/bridge'));
		expect(await mainEnv()).toEqual(before);
		await page.evaluate(() => {
			window.open('https://example.invalid/new-window');
		});
		await expect.poll(() => records(seen, 'xdg-open').length).toBe(2);
		expect(await mainEnv()).toEqual(before);
		expect(await page.evaluate((dir) => window.aniGui.revealInFolder(dir), profile)).toBe(true);
		expect(await mainEnv()).toEqual(before);

		const opened = records(seen, 'xdg-open');
		expect(opened.map((r) => r.args).sort()).toEqual(
			['https://example.invalid/bridge', 'https://example.invalid/new-window', profile].sort()
		);
		for (const r of opened) expectUserDesktop(r.env, `xdg-open ${r.args}`);

		await expect.poll(() => records(seen, 'busctl').length).toBeGreaterThan(0);
		for (const r of records(seen, 'busctl')) expectUserDesktop(r.env, 'busctl (reduced motion)');

		const main = app.process().pid;
		if (main === undefined) throw new Error('the app has no pid');
		const backend = parseEnv(fs.readFileSync(`/proc/${backendPid(main)}/environ`, 'utf-8'));
		expectUserDesktop(backend, 'the backend');
		// What the backend relies on is still there.
		expect(backend.XDG_CONFIG_HOME).toBe(path.join(profile, 'config'));
		expect(backend.ANI_GUI_UNATTENDED).toBe('1');
		expect(backend.http_proxy).toBe(before.http_proxy);
	} finally {
		await stopApp(app);
	}
});
