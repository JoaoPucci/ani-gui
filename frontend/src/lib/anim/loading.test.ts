// The loader's animation is plain keyframes.
//
// lottie-web runs an After Effects expression by handing its source
// to `eval`, which the renderer's content security policy refuses —
// and the overlay loads lottie's light build, which has no expression
// engine at all. An expression left in the JSON would not throw there;
// the property it drives would simply hold still. So the data has to
// carry what the expressions used to compute.
import { describe, expect, it } from 'vitest';

import loading from './loading.json';

/** Every expression source in a Lottie document: a property's `x`
 *  holds one when it is a string (easing handles use `x` too, as
 *  numbers). */
function expressions(node: unknown, path = '$'): string[] {
	if (Array.isArray(node)) return node.flatMap((v, i) => expressions(v, `${path}[${i}]`));
	if (node === null || typeof node !== 'object') return [];
	const found: string[] = [];
	for (const [key, value] of Object.entries(node)) {
		if (key === 'x' && typeof value === 'string') found.push(`${path}.x`);
		else found.push(...expressions(value, `${path}.${key}`));
	}
	return found;
}

describe('the loader animation', () => {
	it('carries no expressions', () => {
		expect(expressions(loading)).toEqual([]);
	});

	it('still moves the pieces the expressions used to shake', () => {
		// The five confetti layers each sit under a null of their own
		// whose position carries the baked wiggle; a null that holds
		// still would mean the motion was dropped, not baked.
		type Layer = { ty: number; nm: string; ks: { p: { a: number; k: { s: number[] }[] } } };
		const assets = (loading as unknown as { assets: { layers?: Layer[] }[] }).assets;
		const layers = assets.flatMap((a) => a.layers ?? []);
		const shakers = layers.filter((l) => l.ty === 3 && / wiggle$/.test(l.nm));
		expect(shakers).toHaveLength(5);
		for (const s of shakers) {
			const p = s.ks.p;
			expect(p.a, `${s.nm} is animated`).toBe(1);
			const xs = new Set(p.k.map((k) => k.s[0]));
			expect(xs.size, `${s.nm} moves`).toBeGreaterThan(10);
		}
	});
});
