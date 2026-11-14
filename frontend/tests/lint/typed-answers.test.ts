// A fake backend answer in test code names the route's type.
//
// The tests fake the backend with MSW, and `HttpResponse.json(body)`
// takes any body: a fake left untyped can drift from what the backend
// sends and keep a test green on a shape no backend produces. With a
// type argument (`HttpResponse.json<KitsuAnimeRef[]>(...)`) MSW checks
// the body against the renderer's type, and svelte-check, which reads
// the test files too, fails the drift. `frontend/tests/answers.ts`
// carries the rest: `errorAnswer` for an error and its envelope, and
// `malformedAnswer` for an answer that is wrong on purpose.
//
// ESLint (eslint.config.js) holds the call's shape in every test file:
// `HttpResponse.json` without a type argument, or with one that types
// nothing (`unknown`, `any`, MSW's `JsonBodyType`), is refused. This
// file runs ESLint over small fixtures to prove the rule is configured
// and fires.
//
// What it does not see: whether the type named is the right route's
// (a handler for one route answering another's type), a `fetch` stub
// built by hand (the unit tests' typed `mockFetchOnce` and its kind
// are held by the compiler, not by this rule), and a body cast to the
// type before the call (`body as KitsuAnimeRef`).

import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

import { ESLint } from 'eslint';
import { describe, expect, it } from 'vitest';

const FRONTEND = fileURLToPath(new URL('../..', import.meta.url));

const head = `import { http, HttpResponse } from 'msw';\nimport type { KitsuAnimeRef } from '$lib/api';\n`;

async function lint(code: string, filePath: string): Promise<string[]> {
	const eslint = new ESLint({ cwd: FRONTEND });
	const [result] = await eslint.lintText(head + code, { filePath: join(FRONTEND, filePath) });
	return result.messages
		.filter((msg) => msg.ruleId === 'no-restricted-syntax')
		.map((msg) => msg.message);
}

/** Paths under frontend/: a scenario, a module of shared handlers, a
 *  unit test. */
const testFiles = [
	'tests/acceptance/a-scenario.test.ts',
	'tests/acceptance/shared-handlers.ts',
	'src/lib/a-module.test.ts'
];

const refused: [string, string][] = [
	['an untyped answer', `export const h = http.get('/x', () => HttpResponse.json([]));\n`],
	[
		'an untyped error answer',
		`export const h = http.get('/x', () => HttpResponse.json({ kind: 'io' }, { status: 500 }));\n`
	],
	['an answer typed unknown', `export const h = () => HttpResponse.json<unknown>([]);\n`],
	[
		"an answer typed as MSW's any body",
		`import type { JsonBodyType } from 'msw';\nexport const h = () => HttpResponse.json<JsonBodyType>([]);\n`
	]
];

describe('typed fake answers', () => {
	it.each(testFiles.flatMap((file) => refused.map(([what, code]) => [what, file, code])))(
		'ESLint refuses %s in %s',
		async (_what, file, code) => {
			const messages = await lint(code, file);
			expect(messages.length).toBeGreaterThan(0);
			expect(messages.join('\n')).toMatch(/tests\/answers/);
		},
		30_000
	);

	it.each(testFiles)(
		'ESLint allows an answer that names its type in %s',
		async (file) => {
			const code = `export const h = http.get('/x', () => HttpResponse.json<KitsuAnimeRef[]>([]));\n`;
			expect(await lint(code, file)).toEqual([]);
		},
		30_000
	);

	it('leaves production code alone', async () => {
		const code = `export const h = () => HttpResponse.json([]);\n`;
		expect(await lint(code, 'src/lib/a-module.ts')).toEqual([]);
	}, 30_000);
});
