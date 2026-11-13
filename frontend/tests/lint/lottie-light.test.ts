// The app loads lottie-web's light build, never the full one.
//
// The full build (`lottie-web`'s main entry) carries the expression
// engine, which compiles each expression with `eval`; the renderer's
// content security policy has no 'unsafe-eval', so an animation with an
// expression would throw on every show. The light build has no engine
// and no `eval`.
//
// A syntactic constraint over the source text: the bare module name
// `'lottie-web'` (or with double quotes or backticks), quote-closed,
// appears in no .ts or .svelte file under src/. A subpath import
// (`'lottie-web/build/player/lottie_light'`) does not match. What it
// does not know: it does not tell comments from code (a comment that
// quotes the bare name fails it), and it would not see the full build
// reached by another subpath (`lottie-web/build/player/lottie_svg`, which
// also carries the engine) — none is imported today.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const SRC = fileURLToPath(new URL('../../src', import.meta.url));

function sources(dir: string): string[] {
	return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
		const p = join(dir, e.name);
		if (e.isDirectory()) return e.name === 'paraglide' ? [] : sources(p);
		return /\.(ts|svelte)$/.test(e.name) ? [p] : [];
	});
}

describe('lottie build', () => {
	it('nothing under src/ imports the full build', () => {
		const offenders = sources(SRC).filter((f) =>
			/(['"`])lottie-web\1/.test(readFileSync(f, 'utf8'))
		);
		expect(offenders).toEqual([]);
	});
});
