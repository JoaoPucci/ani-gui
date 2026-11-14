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
// nothing, is refused: `unknown` or `any` anywhere in it
// (`Record<string, unknown>`, `any[]`, `unknown | null`; no route's type
// holds either), `object` or `{}` anywhere in it, and `Object`, `never`
// or MSW's `JsonBodyType` as the whole type. It also refuses app code
// importing through a path segment named `testing`, which is where the
// shared fakes live (`$lib/testing`): a static import or re-export, a
// dynamic `import()` of a string or template, an `import.meta.glob`
// pattern, and a `new URL(…, import.meta.url)` (which Vite bundles, as
// an asset or a worker) that names the segment. This file runs ESLint over small
// fixtures to prove the rules are configured and fire.
//
// What it does not see, and review holds instead:
// - whether the type named is the right route's (a handler for one
//   route answering another's type);
// - an import of `$lib/testing` whose path does not name the segment:
//   one built at run time (`'../' + 'testing/x'`), or a glob that
//   reaches it through a wildcard (`import.meta.glob('../**/*.ts')`, or
//   a template whose segment is an expression, which Vite reads as one);
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
		'an answer typed as a list of unknown',
		`export const h = () => HttpResponse.json<unknown[]>([]);\n`
	],
	['an answer typed any or null', `export const h = () => HttpResponse.json<any | null>(null);\n`],
	['an answer typed Partial<any>', `export const h = () => HttpResponse.json<Partial<any>>({});\n`],
	['an answer typed a list of object', `export const h = () => HttpResponse.json<object[]>([]);\n`],
	['an answer typed {}', `export const h = () => HttpResponse.json<{}>({});\n`],
	[
		'an answer typed a map of unknown',
		`export const h = () => HttpResponse.json<Record<string, unknown>>({});\n`
	],
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

	it.each(testFiles)(
		'ESLint allows the map a route answers in %s',
		async (file) => {
			const code = `export const h = () => HttpResponse.json<Record<string, number>>({});\n`;
			expect(await lint(code, file)).toEqual([]);
		},
		30_000
	);

	const importsTesting = `import { kitsuRefAnswer } from '$lib/testing/wire-answers';\nexport const x = kitsuRefAnswer();\n`;

	async function restrictedImports(code: string, file: string): Promise<string[]> {
		const eslint = new ESLint({ cwd: FRONTEND });
		const [result] = await eslint.lintText(code, { filePath: join(FRONTEND, file) });
		return result.messages
			.filter(
				(msg) => msg.ruleId === 'no-restricted-imports' || msg.ruleId === 'no-restricted-syntax'
			)
			.map((msg) => msg.message);
	}

	const appImports: [string, string][] = [
		['a static import', importsTesting],
		[
			'a relative import naming the .js',
			`import { kitsuRefAnswer } from '../testing/wire-answers.js';\nexport const x = kitsuRefAnswer();\n`
		],
		['a re-export', `export { kitsuRefAnswer } from '$lib/testing/wire-answers';\n`],
		['a dynamic import', `export const x = () => import('$lib/testing/wire-answers');\n`],
		['a glob over it', `export const x = import.meta.glob('$lib/testing/*.ts');\n`],
		[
			'a relative import of another module there',
			`import { other } from '../testing/other';\nexport const x = other;\n`
		],
		['an import of the directory', `import * as t from '../testing/';\nexport const x = t;\n`],
		['an import through ./testing', `import * as t from './testing';\nexport const x = t;\n`],
		['a glob written as a template', 'export const x = import.meta.glob(`../testing/*.ts`);\n'],
		[
			'a URL Vite bundles',
			`export const x = new URL('../testing/wire-answers.ts', import.meta.url);\n`
		],
		[
			'a worker',
			`export const x = new Worker(new URL('../testing/wire-answers.ts', import.meta.url), { type: 'module' });\n`
		]
	];

	it.each(
		[
			'src/lib/history/a-module.ts',
			'src/lib/motion/a-module.ts',
			'src/routes/+page.svelte'
		].flatMap((file) => appImports.map(([what, code]) => [what, file, code]))
	)(
		'ESLint refuses %s of $lib/testing from app code in %s',
		async (_what, file, code) => {
			const text = file.endsWith('.svelte') ? `<script lang="ts">\n${code}</script>\n` : code;
			expect((await restrictedImports(text, file)).join('\n')).toMatch(/\$lib\/testing/);
			expect(await restrictedImports(text, file)).not.toEqual([]);
		},
		30_000
	);

	it.each([
		['a module whose name ends in testing', `export const x = () => import('./attesting');\n`],
		['a directory whose name ends in testing', `export const x = () => import('./contesting/x');\n`]
	])(
		'ESLint allows %s in app code',
		async (_what, code) => {
			expect(await restrictedImports(code, 'src/lib/history/a-module.ts')).toEqual([]);
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
