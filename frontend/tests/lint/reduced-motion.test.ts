// Nothing in the app animates outside the reduced-motion mechanism.
//
// Under reduced motion the app stands still, and that has to hold for
// the next animation someone adds, not only for the ones that exist
// today. The mechanism has two halves, and each is held here by a
// parser, not by reading source text:
//
// - Stylesheets. `src/lib/design/tokens.css` carries one rule that,
//   under `prefers-reduced-motion: reduce`, takes every CSS animation
//   off every element, zeroes every transition's duration and holds
//   `scroll-behavior` at `auto`, all `!important`. A new `animation` or
//   `transition` needs nothing more to comply: the rule wins over it.
//   What could beat the rule is another `!important` on those
//   properties — in a stylesheet (its specificity, or a cascade layer,
//   would win) or in an inline style. So the check parses every
//   stylesheet with postcss, and every component's markup with
//   Svelte's compiler, and fails on any such `!important` outside the
//   rule's own reduced-motion block. It also parses tokens.css and
//   requires the rule itself.
//
// - Script. Svelte's transitions run through the Web Animations API,
//   which no stylesheet reaches, so every `transition:`, `in:`,
//   `out:` and `animate:` directive must name a function that reads
//   the preference. Svelte's compiler parses each component; the
//   directive's name must be imported from `$lib/motion`, or be a
//   `const` the component builds with `reducible(...)` from there.
//   What `$lib/motion` exports is trusted to read the preference; its
//   own unit tests hold that.
//   The rest of script motion is held by ESLint rules (eslint.config.js),
//   whose parser sees calls and imports: outside `src/lib/motion/`,
//   nothing may import `svelte/transition`, `svelte/animate`,
//   `svelte/motion` or `lottie-web`, call a method named `animate`,
//   pass `behavior: 'smooth'`, or name the reduced-motion media query.
//   The last section below runs ESLint over small fixtures to prove the
//   rules are configured and fire.
//
// What it does not see, deliberately:
//
// - An inline style built from an expression (`style="{css}"`,
//   `style:transition={value}` without `|important`) is not read; only
//   the static text of a `style` attribute and the `|important`
//   modifier of a style directive are. An expression that produces
//   `!important` for a motion property would get past it.
// - Styles set from script (`el.style.setProperty(..., 'important')`,
//   `el.style.cssText = ...`) are not read.
// - Script motion that is not one of the named forms: a
//   `requestAnimationFrame` or `setInterval` loop that moves something
//   by hand, a method called through a computed name
//   (`el['animate']`), a scroll behaviour passed as a variable, or a
//   third-party animation library other than lottie-web. Whether a
//   frame loop is motion is a question about what it does, which no
//   syntactic rule can answer; such code reads `reducedMotion` from
//   `$lib/motion/reduced-motion.svelte` by convention, and review holds
//   it there.
// - Motion in media: an animated image or a video's own content.
//
// The runtime half — that nothing is actually running under reduced
// motion — is asserted on the main surfaces by the acceptance
// scenarios (`tests/acceptance/reduced-motion-*.test.ts`) and, with
// real CSS, by the Electron e2e (`electron/e2e/reduced-motion.spec.ts`).

import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { ESLint } from 'eslint';
import postcss, { type AtRule, type Container, type Declaration, type Root } from 'postcss';
import { parse } from 'svelte/compiler';
import { describe, expect, it } from 'vitest';

const FRONTEND = fileURLToPath(new URL('../..', import.meta.url));
const SRC = join(FRONTEND, 'src');
const MOTION_DIR = join(SRC, 'lib', 'motion');
const TOKENS = join(SRC, 'lib', 'design', 'tokens.css');
const GENERATED = join(SRC, 'lib', 'paraglide');

const files = (readdirSync(SRC, { recursive: true }) as string[])
	.map((rel) => join(SRC, rel))
	.filter((path) => !path.startsWith(GENERATED));
const components = files.filter((path) => path.endsWith('.svelte'));
const stylesheets = files.filter((path) => path.endsWith('.css'));
const texts = new Map(
	[...components, ...stylesheets].map((path) => [path, readFileSync(path, 'utf8')])
);

const MOTION_PROPERTY = /^(animation|transition|scroll-behavior)(-|$)/;
const REDUCE_QUERY = /prefers-reduced-motion\s*:\s*reduce/;
/** The values a reduced-motion block may force: each one stops motion. */
const STOPPING_VALUE = /^(none|0m?s|auto)\s*$/;

const lineOf = (text: string, offset: number) => text.slice(0, offset).split('\n').length;
const where = (path: string, line: number) => `${relative(SRC, path)}:${line}`;

// ---- Svelte: what each component's markup and styles contain --------------

type SvelteAst = ReturnType<typeof parse>;
interface AstNode {
	type: string;
	start: number;
	end: number;
	[key: string]: unknown;
}

function walk(node: unknown, visit: (node: AstNode) => void): void {
	if (Array.isArray(node)) {
		for (const child of node) walk(child, visit);
		return;
	}
	if (!node || typeof node !== 'object') return;
	const n = node as AstNode;
	if (typeof n.type === 'string') visit(n);
	for (const [key, value] of Object.entries(n)) {
		if (key === 'parent' || key === 'metadata') continue;
		if (value && typeof value === 'object') walk(value, visit);
	}
}

const asts = new Map<string, SvelteAst>(
	components.map((path) => [path, parse(texts.get(path)!, { modern: true, filename: path })])
);

/** Whether an import source names a module under `src/lib/motion/`. */
function isMotionSource(source: string, from: string): boolean {
	if (source === '$lib/motion' || source.startsWith('$lib/motion/')) return true;
	if (!source.startsWith('.')) return false;
	const target = resolve(dirname(from), source);
	return target === MOTION_DIR || target.startsWith(MOTION_DIR + '/');
}

/** The names a component may give a motion directive: what it imports
 *  from `$lib/motion`, and each `const` it builds with `reducible(...)`
 *  imported from there. */
function compliantNames(path: string, ast: SvelteAst): Set<string> {
	const motionImports = new Map<string, string>();
	const scripts = [ast.instance?.content, ast.module?.content];
	for (const script of scripts) {
		walk(script, (node) => {
			if (node.type !== 'ImportDeclaration') return;
			const source = (node.source as { value: string }).value;
			if (!isMotionSource(source, path)) return;
			for (const spec of node.specifiers as {
				type: string;
				local: { name: string };
				imported?: { name: string };
			}[]) {
				motionImports.set(spec.local.name, spec.imported?.name ?? spec.local.name);
			}
		});
	}
	const names = new Set(motionImports.keys());
	for (const script of scripts) {
		walk(script, (node) => {
			if (node.type !== 'VariableDeclaration' || node.kind !== 'const') return;
			walk(node.declarations, (node) => {
				if (node.type !== 'VariableDeclarator') return;
				const id = node.id as { type: string; name?: string };
				const init = node.init as {
					type?: string;
					callee?: { type: string; name?: string };
				} | null;
				if (id.type !== 'Identifier' || init?.type !== 'CallExpression') return;
				const callee = init.callee;
				if (callee?.type !== 'Identifier') return;
				if (motionImports.get(callee.name!) === 'reducible') names.add(id.name!);
			});
		});
	}
	return names;
}

// ---- The three checks, each over one file -----------------------------------

function insideReduceBlock(decl: Declaration): boolean {
	for (let p = decl.parent as Container | undefined; p; p = p.parent as Container | undefined) {
		if (
			p.type === 'atrule' &&
			(p as AtRule).name === 'media' &&
			REDUCE_QUERY.test((p as AtRule).params)
		)
			return true;
	}
	return false;
}

/** `!important` motion declarations in a stylesheet that could beat the
 *  stop rule: any outside a reduced-motion block, and any inside one
 *  that forces something other than a stop. */
function stylesheetOffenders(path: string, text: string, css: string, offset: number): string[] {
	const offenders: string[] = [];
	postcss.parse(css).walkDecls((decl) => {
		if (!decl.important || !MOTION_PROPERTY.test(decl.prop)) return;
		if (insideReduceBlock(decl) && STOPPING_VALUE.test(decl.value)) return;
		const line = lineOf(text, offset + (decl.source?.start?.offset ?? 0));
		offenders.push(`${where(path, line)} ${decl.prop}: ${decl.value} !important`);
	});
	return offenders;
}

/** `!important` motion properties in a component's inline styles. */
function inlineOffenders(path: string, text: string, ast: SvelteAst): string[] {
	const offenders: string[] = [];
	walk(ast.fragment, (node) => {
		const at = where(path, lineOf(text, node.start));
		if (node.type === 'StyleDirective') {
			const modifiers = (node.modifiers as string[]) ?? [];
			if (MOTION_PROPERTY.test(node.name as string) && modifiers.includes('important')) {
				offenders.push(`${at} style:${node.name}|important`);
			}
			return;
		}
		if (node.type !== 'Attribute' || node.name !== 'style' || !Array.isArray(node.value)) return;
		const statics = (node.value as { type: string; data?: string }[])
			.filter((part) => part.type === 'Text')
			.map((part) => part.data ?? '')
			.join('');
		let decls: Root;
		try {
			decls = postcss.parse(`x{${statics}}`);
		} catch {
			return; // Static text an expression broke apart; not readable on its own.
		}
		decls.walkDecls((decl) => {
			if (decl.important && MOTION_PROPERTY.test(decl.prop)) {
				offenders.push(`${at} style="${decl.prop}: ${decl.value} !important"`);
			}
		});
	});
	return offenders;
}

/** Motion directives whose function does not come from `$lib/motion`. */
function directiveOffenders(path: string, text: string, ast: SvelteAst): string[] {
	const allowed = compliantNames(path, ast);
	const offenders: string[] = [];
	walk(ast.fragment, (node) => {
		if (node.type !== 'TransitionDirective' && node.type !== 'AnimateDirective') return;
		if (allowed.has((node.name as string).split('.')[0])) return;
		const source = text.slice(node.start, node.end).split('=')[0];
		offenders.push(`${where(path, lineOf(text, node.start))} ${source}`);
	});
	return offenders;
}

function componentStyle(ast: SvelteAst): { css: string; offset: number } | null {
	return ast.css ? { css: ast.css.content.styles, offset: ast.css.content.start } : null;
}

describe('reduced motion: the sweep', () => {
	it('found the components and the stylesheets', () => {
		// An empty walk must not read as a clean one.
		expect(stylesheets).toContain(TOKENS);
		expect(components.length).toBeGreaterThan(20);
		expect([...asts.values()].filter(componentStyle).length).toBeGreaterThan(20);
		let directives = 0;
		for (const ast of asts.values()) {
			walk(ast.fragment, (node) => {
				if (node.type === 'TransitionDirective' || node.type === 'AnimateDirective') directives++;
			});
		}
		expect(directives).toBeGreaterThan(5);
	});
});

describe('reduced motion: stylesheets', () => {
	it('tokens.css stops every CSS animation, transition and smooth scroll under reduced motion', () => {
		const forced = new Map<string, string>();
		postcss.parse(texts.get(TOKENS)!).walkAtRules('media', (media) => {
			if (!REDUCE_QUERY.test(media.params)) return;
			media.walkRules((rule) => {
				const selectors = rule.selectors.map((sel) => sel.trim());
				if (!['*', '*::before', '*::after'].every((sel) => selectors.includes(sel))) return;
				rule.walkDecls((decl) => {
					if (decl.important) forced.set(decl.prop, decl.value.trim());
				});
			});
		});
		expect(Object.fromEntries(forced)).toEqual({
			animation: 'none',
			'transition-duration': '0s',
			'scroll-behavior': 'auto'
		});
	});

	it('no stylesheet forces a motion property past that rule', () => {
		const offenders = [
			...stylesheets.flatMap((path) =>
				stylesheetOffenders(path, texts.get(path)!, texts.get(path)!, 0)
			),
			...components.flatMap((path) => {
				const style = componentStyle(asts.get(path)!);
				return style ? stylesheetOffenders(path, texts.get(path)!, style.css, style.offset) : [];
			})
		];
		expect(
			offenders,
			`motion forced past the reduced-motion rule:\n${offenders.join('\n')}`
		).toEqual([]);
	});

	it('no inline style forces a motion property past that rule', () => {
		const offenders = components.flatMap((path) =>
			inlineOffenders(path, texts.get(path)!, asts.get(path)!)
		);
		expect(
			offenders,
			`inline motion forced past the reduced-motion rule:\n${offenders.join('\n')}`
		).toEqual([]);
	});

	// The sweep above passes on a clean tree, which is also what a check
	// that never fires looks like. These run the same functions over
	// fixtures that must fail it, and ones that must not.
	const fixture = join(SRC, 'routes', '__fixture.svelte');
	const sheetOf = (css: string) => stylesheetOffenders(fixture, css, css, 0);
	const inlineOf = (markup: string) =>
		inlineOffenders(fixture, markup, parse(markup, { modern: true }));

	it.each([
		['an animation', '.x { animation: spin 1s linear infinite !important; }'],
		['a transition longhand', '.x { transition-duration: 300ms !important; }'],
		['smooth scrolling', 'html { scroll-behavior: smooth !important; }'],
		['a layered rule', '@layer motion { .x { transition: opacity 1s !important; } }'],
		[
			'motion forced inside a reduced-motion block',
			'@media (prefers-reduced-motion: reduce) { .x { animation: pulse 2s infinite !important; } }'
		]
	])('flags %s forced with !important', (_what, css) => {
		expect(sheetOf(css)).toHaveLength(1);
	});

	it.each([
		['an ordinary animation', '.x { animation: spin 1s linear infinite; }'],
		['an important colour', '.x { color: red !important; }'],
		[
			'a stop inside a reduced-motion block',
			'@media (prefers-reduced-motion: reduce) { .x { animation: none !important; transition-duration: 0s !important; } }'
		]
	])('lets %s through', (_what, css) => {
		expect(sheetOf(css)).toEqual([]);
	});

	it('flags an inline motion style forced with !important, in either form', () => {
		expect(inlineOf('<div style="transition: opacity 1s !important"></div>')).toHaveLength(1);
		expect(inlineOf('<div style:animation|important={value}></div>')).toHaveLength(1);
		expect(inlineOf('<div style="color: red !important" style:transition={value}></div>')).toEqual(
			[]
		);
	});
});

describe('reduced motion: Svelte directives', () => {
	it('every transition and animate directive goes through $lib/motion', () => {
		const offenders = components.flatMap((path) =>
			directiveOffenders(path, texts.get(path)!, asts.get(path)!)
		);
		expect(
			offenders,
			`directives that do not read the motion preference — use $lib/motion/transitions:\n${offenders.join('\n')}`
		).toEqual([]);
	});

	const component = (script: string, markup: string) =>
		`<script lang="ts">\n${script}\n</script>\n${markup}\n`;
	const offendersIn = (path: string, source: string) =>
		directiveOffenders(path, source, parse(source, { modern: true }));
	const route = join(SRC, 'routes', '__fixture.svelte');
	const libComponent = join(SRC, 'lib', 'components', '__fixture.svelte');

	it.each([
		[
			'Svelte’s own transition',
			`import { fade } from 'svelte/transition';`,
			'<div transition:fade></div>'
		],
		['Svelte’s own FLIP', `import { flip } from 'svelte/animate';`, '<div animate:flip></div>'],
		[
			'a local function',
			`const pop = (node: Element) => ({ duration: 200 });`,
			'<div in:pop></div>'
		],
		[
			'a reducible() held in a let, which could be reassigned',
			`import { reducible, scale } from '$lib/motion/transitions';\nlet pop = reducible((node: Element) => scale(node, {}));\npop = (node: Element) => ({ duration: 200 });`,
			'<div out:pop></div>'
		],
		[
			'a local wrapper around a motion transition',
			`import { scale } from '$lib/motion/transitions';\nconst pop = (node: Element) => scale(node, { duration: 200 });`,
			'<div out:pop></div>'
		]
	])('flags %s', (_what, script, markup) => {
		expect(offendersIn(route, component(script, markup))).toHaveLength(1);
	});

	it.each([
		[
			'an import from $lib/motion',
			`import { fly } from '$lib/motion/transitions';`,
			'<div transition:fly={{ y: 8 }}></div>'
		],
		[
			'a renamed import',
			`import { fly as rise } from '$lib/motion/transitions';`,
			'<div in:rise></div>'
		],
		[
			'a reducible() built in the component',
			`import { reducible, scale } from '$lib/motion/transitions';\nconst pop = reducible((node: Element) => scale(node, { duration: 200 }));`,
			'<div out:pop></div>'
		]
	])('lets %s through', (_what, script, markup) => {
		expect(offendersIn(route, component(script, markup))).toEqual([]);
	});

	it('follows a relative import into $lib/motion', () => {
		const source = component(
			`import { slide } from '../motion/transitions';`,
			'<div transition:slide></div>'
		);
		expect(offendersIn(libComponent, source)).toEqual([]);
		expect(offendersIn(route, source)).toHaveLength(1);
	});
});

describe('reduced motion: script', () => {
	// Every case is linted as a module outside src/lib/motion/, where the
	// rules apply, and the clean case inside it, where they do not.
	const outside = join(SRC, 'routes', '__reduced-motion-fixture.ts');
	const inside = join(MOTION_DIR, '__reduced-motion-fixture.ts');
	const cases: [string, string][] = [
		['a Svelte transition', `import { fade } from 'svelte/transition';\nexport { fade };\n`],
		['Svelte FLIP', `import { flip } from 'svelte/animate';\nexport { flip };\n`],
		['a Svelte tween or spring', `import { Tween } from 'svelte/motion';\nexport { Tween };\n`],
		['lottie-web, statically', `import lottie from 'lottie-web';\nexport { lottie };\n`],
		['lottie-web, lazily', `export const load = () => import('lottie-web');\n`],
		['a Web Animation', `export const go = (el: Element) => el.animate([], 300);\n`],
		[
			'a gliding scroll',
			`export const go = (el: Element) => el.scrollIntoView({ behavior: 'smooth' });\n`
		],
		['the media query itself', `export const q = '(prefers-reduced-motion: reduce)';\n`]
	];

	async function lint(code: string, filePath: string): Promise<string[]> {
		const eslint = new ESLint({ cwd: FRONTEND });
		const [result] = await eslint.lintText(code, { filePath });
		return result.messages
			.filter(
				(msg) => msg.ruleId === 'no-restricted-imports' || msg.ruleId === 'no-restricted-syntax'
			)
			.map((msg) => msg.message);
	}

	it.each(cases)(
		'ESLint refuses %s outside $lib/motion',
		async (_what, code) => {
			const messages = await lint(code, outside);
			expect(messages.length).toBeGreaterThan(0);
			expect(messages.join('\n')).toMatch(/\$lib\/motion/);
		},
		30_000
	);

	it.each(cases)(
		'ESLint allows %s inside $lib/motion',
		async (_what, code) => {
			expect(await lint(code, inside)).toEqual([]);
		},
		30_000
	);
});

// An indeterminate progress bar is a short chunk the animation sweeps
// along its track. With the animation taken off, the chunk sits at the
// track's start and reads as determinate progress stuck part-way. Each
// one therefore names, under reduced motion, a still full-width bar.
// This is a syntactic constraint over the bars that exist: the listed
// selector must appear in a reduced-motion block of its file and set
// `inline-size: 100%` there. A new indeterminate bar has to be added to
// the list; nothing here finds one by itself.
describe('reduced motion: indeterminate progress bars', () => {
	const bars: [string, string][] = [
		['lib/components/SearchProgress.svelte', '.search-progress-bar'],
		['routes/play/[id]/+page.svelte', '.player-spinner-sweep'],
		['lib/components/DownloadBar.svelte', '.dl-bar-progress span'],
		['lib/components/DownloadDock.svelte', '.dl-row-bar-indet span']
	];

	it.each(bars)('%s holds %s still at full width', (file, selector) => {
		const path = join(SRC, file);
		const style = componentStyle(asts.get(path)!);
		expect(style, `${file} has no <style>`).not.toBeNull();
		const widths: string[] = [];
		postcss.parse(style!.css).walkRules((rule) => {
			if (!rule.selectors.map((sel) => sel.trim()).includes(selector)) return;
			rule.walkDecls('inline-size', (decl) => {
				if (insideReduceBlock(decl)) widths.push(decl.value.trim());
			});
		});
		expect(widths).toEqual(['100%']);
	});
});
