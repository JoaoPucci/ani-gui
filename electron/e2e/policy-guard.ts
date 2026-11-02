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
 *     too, before Playwright has attached anything to the page;
 *   - a `securitypolicyviolation` listener installed on every later
 *     document, which reports through the console as well;
 *   - the page's console and its uncaught errors (a refused `eval`
 *     throws an EvalError).
 *
 * Specs import `test` and `expect` from here instead of
 * '@playwright/test'; the shared launch helper (./launch.ts) guards
 * every app it starts. After each test, a line that matches none of the
 * patterns the test declared with `expectViolation` fails it. A test
 * that declares some gets its app's guard from `guardPolicy(app)`.
 */
import { test as base, expect, type ElectronApplication } from '@playwright/test';

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

/** Listen for violations on `app` for the rest of the current test; the
 *  same guard again for an app already guarded. */
export function guardPolicy(app: ElectronApplication): PolicyGuard {
	const known = byApp.get(app);
	if (known) return known;
	const seen: string[] = [];
	const declared: RegExp[] = [];
	const note = (line: string) => {
		if (VIOLATION.test(line)) seen.push(line.trim());
	};
	const proc = app.process();
	for (const stream of [proc.stdout, proc.stderr]) {
		let buf = '';
		stream?.on('data', (chunk: Buffer) => {
			buf += chunk.toString('utf8');
			let nl: number;
			while ((nl = buf.indexOf('\n')) >= 0) {
				note(buf.slice(0, nl));
				buf = buf.slice(nl + 1);
			}
		});
	}
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
	const guard: PolicyGuard = {
		seen,
		expectViolation: (pattern) => declared.push(pattern)
	};
	guards.push({ guard, declared });
	byApp.set(app, guard);
	return guard;
}

/** Undeclared violations reported so far, across every guarded app. */
function undeclared(): string[] {
	return guards.flatMap(({ guard, declared }) =>
		guard.seen.filter((line) => !declared.some((p) => p.test(line)))
	);
}

export const test = base.extend<{ policyGuard: void }>({
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
