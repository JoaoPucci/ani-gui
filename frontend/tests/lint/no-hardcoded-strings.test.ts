import { fileURLToPath } from 'node:url';
import { ESLint, Linter } from 'eslint';
import svelte from 'eslint-plugin-svelte';
import ts from 'typescript-eslint';
import { describe, expect, it } from 'vitest';
import noHardcodedStrings from '../../tools/eslint-rules/no-hardcoded-strings.js';

// Runs the local rule through ESLint's own Linter with the same Svelte
// parser setup eslint.config.js uses, so a case here behaves the way
// it would in `pnpm run lint`. Each case is a whole component; the
// assertion is on which strings the rule reports.

const linter = new Linter({ configType: 'flat' });
const config = [
	...svelte.configs.recommended,
	{
		files: ['**/*.svelte'],
		languageOptions: { parserOptions: { parser: ts.parser } },
		plugins: { local: { rules: { 'no-hardcoded-strings': noHardcodedStrings } } },
		rules: { 'local/no-hardcoded-strings': 'error' as const }
	}
];

function flagged(source: string): string[] {
	const messages = linter.verify(source, config, 'Fixture.svelte');
	const fatal = messages.find((msg) => msg.fatal);
	if (fatal) throw new Error(`fixture did not parse: ${fatal.message}`);
	return messages
		.filter((msg) => msg.ruleId === 'local/no-hardcoded-strings')
		.map((msg) => msg.message);
}

function component(script: string, markup = ''): string {
	return `<script lang="ts">\n${script}\n</script>\n${markup}\n`;
}

describe('no-hardcoded-strings: template text and attributes', () => {
	it('flags raw text and a literal user-facing attribute', () => {
		const out = flagged(component('', '<button title="Close the panel">Save changes</button>'));
		expect(out).toHaveLength(2);
	});

	it('leaves Paraglide calls alone', () => {
		expect(
			flagged(component("import { m } from '$lib/paraglide/messages';", '<p>{m.x()}</p>'))
		).toEqual([]);
	});
});

describe('no-hardcoded-strings: literals that reach the template through script', () => {
	it('flags a sentence returned from a script helper and rendered by a mustache', () => {
		// The shape that let "Couldn't reach Kitsu." ship: the literal
		// lives in <script>, and the template renders only `{x.headline}`.
		const src = component(
			`function describe() {\n\treturn { headline: "Couldn't reach Kitsu.", detail: null };\n}\nconst error = describe();`,
			'<p>{error.headline}</p>'
		);
		expect(flagged(src).join('\n')).toMatch(/Couldn't reach Kitsu\./);
	});

	it('flags a capitalised label in a script table and a template-literal message', () => {
		const src = component(
			"const QUALITIES = [{ key: 'best', label: 'Best' }];\nlet n = 2;\nconst label = `Track ${n}`;",
			'{#each QUALITIES as q (q.key)}<span>{q.label}</span>{/each}<span>{label}</span>'
		);
		const out = flagged(src).join('\n');
		expect(out).toMatch(/Best/);
		expect(out).toMatch(/Track/);
	});

	it('flags a literal written directly inside a template expression', () => {
		expect(flagged(component('', "<p>{'Nothing to show here.'}</p>")).join('\n')).toMatch(
			/Nothing to show here/
		);
	});

	it('does not flag identifiers, keys, comparisons, imports, logs or thrown errors', () => {
		const src = component(
			[
				"import Thing from './Thing.svelte';",
				"let mode = $state<'Sub' | 'Dub'>('sub');",
				"const keyed = { 'Content-Type': 'application/json' };",
				"if (mode === 'Sub') console.warn('Something odd happened.');",
				"function onKey(e: KeyboardEvent) { if (e.key === 'Escape') mode = 'sub'; }",
				"const css = 'translate3d(0, 0, 0) scale(1)';",
				"function fail() { throw new Error('Stream closed before resolution finished.'); }"
			].join('\n')
		);
		expect(flagged(src)).toEqual([]);
	});

	it('honours an i18n-ignore comment on the preceding line', () => {
		const src = component("// i18n-ignore: proper noun\nconst provider = 'AniList';");
		expect(flagged(src)).toEqual([]);
	});
});

describe('no-hardcoded-strings: where the project applies it', () => {
	// Copy also lives in plain TypeScript modules — a label table, a
	// credits list, a message picked by a helper — that a component only
	// renders. These run the project's own eslint.config.js, so they pin
	// which files the rule reaches, not just what it reports.
	const frontendDir = fileURLToPath(new URL('../..', import.meta.url));
	const eslint = new ESLint({ cwd: frontendDir });

	async function ruleHits(relPath: string, code: string): Promise<string[]> {
		const [result] = await eslint.lintText(code, { filePath: `${frontendDir}${relPath}` });
		return result.messages
			.filter((msg) => msg.ruleId === 'local/no-hardcoded-strings')
			.map((msg) => msg.message);
	}

	const COPY = "export const label = 'Loading animation (LottieFiles)';\n";

	it('reaches plain .ts modules under src/', async () => {
		expect(await ruleHits('src/lib/fixture/copy.ts', COPY)).toHaveLength(1);
		expect(await ruleHits('src/routes/fixture/+page.ts', COPY)).toHaveLength(1);
	});

	it('leaves test files alone', async () => {
		expect(await ruleHits('src/lib/fixture/copy.test.ts', COPY)).toEqual([]);
	});

	it('honours i18n-ignore in a .ts module', async () => {
		const ignored = "// i18n-ignore: proper noun\nexport const provider = 'AniList';\n";
		expect(await ruleHits('src/lib/fixture/copy.ts', ignored)).toEqual([]);
	});
});
