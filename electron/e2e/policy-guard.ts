/**
 * Every e2e test fails on a content security policy violation it did
 * not declare.
 *
 * The renderer runs under a policy (electron/lib/renderer-csp.cjs and
 * the bundle's <meta>), and a violation is silent to the user: a
 * poster that never loads, a stream that never attaches, a loader that
 * never moves. So the suite listens for them on every launch:
 *
 *   - the app's own stdout and stderr, where the main process forwards
 *     every renderer console line (`console-message`). Chromium writes
 *     each refusal to the console, and this path sees the first load
 *     too, before Playwright has attached anything to the page. (It is
 *     attached once the launch resolves; the first page loads only
 *     after the backend's handshake, which comes later.)
 *   - a `securitypolicyviolation` listener installed on every later
 *     document, which reports through the console as well;
 *   - the page's console and its uncaught errors (a refused `eval`
 *     throws an EvalError).
 *
 * Specs import `test` and `expect` from here instead of
 * '@playwright/test'; the shared launch helper (./launch.ts) guards
 * every app it starts — through all three paths for a launch through
 * Playwright's driver, through its output alone for spawnPackagedApp's
 * plain child process, which Playwright does not attach to. After each
 * test, a line that matches none of the patterns the test declared
 * with `expectViolation` fails it. A test that declares some gets its
 * app's guard from `guardPolicy(app)`.
 */
import { test as base, expect, type ElectronApplication } from '@playwright/test';
import type { ChildProcess } from 'node:child_process';
import { outputRecorder } from '../lib/app-output.cjs';

export { expect };

/** A line that reports a policy refusal, in any of the shapes above. */
const VIOLATION =
	/\[csp-violation\]|Content Security Policy|Content-Security-Policy|Refused to (load|connect|execute|apply|evaluate|frame|create|compile)/i;

export interface PolicyGuard {
	/** Every reported violation line, declared or not. */
	readonly seen: string[];
	/** Excuse the violations matching `pattern` for the rest of the test. */
	expectViolation(pattern: RegExp): void;
}

const guards: { guard: PolicyGuard; declared: RegExp[] }[] = [];
const byApp = new WeakMap<ElectronApplication, PolicyGuard>();

/** A guard for the rest of the current test, and the function that
 *  records a line in it when the line reports a violation. */
function newGuard(): { guard: PolicyGuard; note: (line: string) => void } {
	const seen: string[] = [];
	const declared: RegExp[] = [];
	const guard: PolicyGuard = {
		seen,
		expectViolation: (pattern) => declared.push(pattern)
	};
	guards.push({ guard, declared });
	return {
		guard,
		note: (line) => {
			if (VIOLATION.test(line)) seen.push(line.trim());
		}
	};
}

/** Feed every line `streams` carry to `note`. */
function readLines(streams: (NodeJS.ReadableStream | null | undefined)[], note: (line: string) => void): void {
	for (const stream of streams) {
		let buf = '';
		stream?.on('data', (chunk: Buffer) => {
			buf += chunk.toString('utf8');
			let nl: number;
			while ((nl = buf.indexOf('\n')) >= 0) {
				note(buf.slice(0, nl));
				buf = buf.slice(nl + 1);
			}
		});
		// A last line the app printed with no newline after it.
		stream?.on('end', () => note(buf));
	}
}

/** Listen for violations in what `streams` carry — the stdout and
 *  stderr of an app Playwright's driver did not start — for the rest of
 *  the current test. */
export function guardOutput(streams: (NodeJS.ReadableStream | null | undefined)[]): PolicyGuard {
	const { guard, note } = newGuard();
	readLines(streams, note);
	return guard;
}

/** Listen for violations on `app` for the rest of the current test; the
 *  same guard again for an app already guarded. */
export function guardPolicy(app: ElectronApplication): PolicyGuard {
	const known = byApp.get(app);
	if (known) return known;
	const { guard, note } = newGuard();
	const proc = app.process();
	readLines([proc.stdout, proc.stderr], note);
	void app
		.context()
		.addInitScript(() => {
			document.addEventListener(
				'securitypolicyviolation',
				(e) =>
					console.error(
						'[csp-violation] ' +
							JSON.stringify({
								directive: e.effectiveDirective,
								blocked: e.blockedURI,
								source: e.sourceFile,
								line: e.lineNumber,
								disposition: e.disposition
							})
					),
				true
			);
		})
		.catch(() => {});
	const watchPage = (page: import('@playwright/test').Page) => {
		page.on('console', (msg) => note(msg.text()));
		page.on('pageerror', (err) => note(`${err.name}: ${err.message}`));
	};
	for (const page of app.windows()) watchPage(page);
	app.on('window', watchPage);
	byApp.set(app, guard);
	return guard;
}

/** Undeclared violations reported so far, across every guarded app. */
function undeclared(): string[] {
	return guards.flatMap(({ guard, declared }) =>
		guard.seen.filter((line) => !declared.some((p) => p.test(line)))
	);
}

/** What each app launched inside the current test said, and how it
 *  ended (lib/app-output.cjs). An app launched in a hook outside any
 *  test (a beforeAll) is not kept: no spec launches one there. */
const outputs: ReturnType<typeof outputRecorder>[] = [];

/** How many of a record's last lines a failing test prints to its
 *  stderr, which the list reporter shows in full in the CI log; the
 *  whole record goes in the HTML report. The list reporter cuts an
 *  attachment it prints to its first 300 characters, which would show
 *  the oldest lines and not the ones near the app's end. */
const PRINTED_LINES = 150;

/** Keep what `child` — a process the shared launch helper started —
 *  writes to its stdout and stderr, and how it ends, for the report of
 *  the current test should it fail (the appOutput fixture below).
 *  `name` says what the process is (the app, or the program it was
 *  started under); `since` says from when on its output was read, for
 *  a child whose first lines went before it was handed over. A stream
 *  that is not a pipe this process reads is not seen, and the record
 *  says so. */
export function recordAppOutput(
	child: ChildProcess,
	{ name = 'app', since }: { name?: string; since?: string } = {}
): void {
	const unread = (['stdout', 'stderr'] as const).filter((stream) => !child[stream]);
	const notes = [since, ...unread.map((stream) => `${stream} not read`)].filter(Boolean);
	const record = outputRecorder(`${name} pid ${child.pid ?? '?'}${notes.length ? ` (${notes.join('; ')})` : ''}`);
	outputs.push(record);
	for (const stream of ['stdout', 'stderr'] as const) {
		child[stream]?.on('data', (chunk: Buffer) => record.feed(stream, chunk));
		child[stream]?.on('end', () => record.end(stream));
	}
	if (child.exitCode !== null || child.signalCode !== null) record.exited(child.exitCode, child.signalCode);
	child.once('exit', (code, signal) => record.exited(code, signal));
	// A spawn that failed emits this, and no exit.
	child.once('error', (err) => {
		if (child.pid === undefined) record.failed(err);
	});
}

export const test = base.extend<{ policyGuard: void; appOutput: void }>({
	// A test that did not end as expected carries, in its report, what
	// each app it launched said and how that app ended: the main
	// process's record of the boot, the backend's log, an exit code or
	// a signal. Its error alone is often only what Playwright saw — an
	// app that quit on its own leaves "Execution context was destroyed"
	// and nothing about why.
	//
	// Declared before policyGuard on purpose: auto fixtures are torn down
	// in the reverse of their order here, so this one runs after the
	// policy guard's check and sees the test fail by it too.
	appOutput: [
		// eslint-disable-next-line no-empty-pattern
		async ({}, use, testInfo) => {
			outputs.length = 0;
			await use();
			const records = outputs.splice(0);
			if (testInfo.status === testInfo.expectedStatus) return;
			for (const [i, record] of records.entries()) {
				const title = `app output ${i + 1} of ${records.length}`;
				process.stderr.write(`\n── ${title} ──\n${record.text({ last: PRINTED_LINES })}\n`);
				await testInfo.attach(title, { body: record.text(), contentType: 'text/plain' });
			}
		},
		{ auto: true }
	],
	policyGuard: [
		// eslint-disable-next-line no-empty-pattern
		async ({}, use) => {
			guards.length = 0;
			await use();
			const lines = [...new Set(undeclared())];
			guards.length = 0;
			expect(lines, `content security policy violations:\n${lines.join('\n')}`).toEqual([]);
		},
		{ auto: true }
	]
});
