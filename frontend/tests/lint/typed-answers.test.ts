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
// nothing (`unknown`, `any`, `object`, `Object`, `never`, MSW's
// `JsonBodyType`), is refused. It also refuses an import of
// `$lib/testing`, where the shared fakes live, from app code. This file
// runs ESLint over small fixtures to prove the rules are configured and
// fire.
//
// What it does not see, and review holds instead:
// - whether the type named is the right route's (a handler for one
//   route answering another's type), or a type broader than the
//   route's (`Record<string, unknown>`, which is also the honest type
//   of a route that answers a map);
// - a body cast before the call (`body as KitsuAnimeRef`), or a call
//   reached another way (`HttpResponse['json'](...)`, an alias of it);
// - an answer built without `HttpResponse.json`: `new HttpResponse` or
//   `HttpResponse.text` with a JSON string or a stream, `Response.json`
//   and `new Response(...)` in a `fetch` stub, and stream event data
//   that does not go through `wire<T>`. The shared helpers for these
//   take a type, and the tests use them, but nothing refuses one that
//   does not;
// - an untyped `vi.fn()` standing in for an API function or a
//   dependency, which is typed by convention.

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
	['an answer typed object', `export const h = () => HttpResponse.json<object>({});\n`],
	['an answer typed never', `export const h = () => HttpResponse.json<never>([] as never);\n`],
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

	const importsTesting = `import { kitsuRefAnswer } from '$lib/testing/wire-answers';\nexport const x = kitsuRefAnswer();\n`;

	async function restrictedImports(code: string, file: string): Promise<string[]> {
		const eslint = new ESLint({ cwd: FRONTEND });
		const [result] = await eslint.lintText(code, { filePath: join(FRONTEND, file) });
		return result.messages
			.filter((msg) => msg.ruleId === 'no-restricted-imports')
			.map((msg) => msg.message);
	}

	it.each(['src/lib/history/a-module.ts', 'src/lib/motion/a-module.ts', 'src/routes/+page.svelte'])(
		'ESLint refuses an import of $lib/testing from app code in %s',
		async (file) => {
			const code = file.endsWith('.svelte')
				? `<script lang="ts">\n${importsTesting}</script>\n`
				: importsTesting;
			expect((await restrictedImports(code, file)).join('\n')).toMatch(/\$lib\/testing/);
		},
		30_000
	);

	it.each(testFiles)(
		'ESLint allows an import of $lib/testing in %s',
		async (file) => {
			expect(await restrictedImports(importsTesting, file)).toEqual([]);
		},
		30_000
	);

	it('leaves production code alone', async () => {
		const code = `export const h = () => HttpResponse.json([]);\n`;
		expect(await lint(code, 'src/lib/a-module.ts')).toEqual([]);
	}, 30_000);
});
