/**
 * Strongly connected components of a run's links (Tarjan): members of a
 * component larger than one lead round to one another, a cycle.
 * Part of the mockups' ordering; `./order` gathers the pieces.
 */

/** Tarjan's components; members of a component larger than one form a cycle. */
export function components(
	ids: readonly string[],
	out: Map<string, string[]>
): Map<string, number> {
	const index = new Map<string, number>();
	const low = new Map<string, number>();
	const stack: string[] = [];
	const onStack = new Set<string>();
	const comp = new Map<string, number>();
	let counter = 0;
	let compCount = 0;
	const visit = (v: string) => {
		index.set(v, counter);
		low.set(v, counter);
		counter++;
		stack.push(v);
		onStack.add(v);
		for (const w of out.get(v) ?? []) {
			if (!index.has(w)) {
				visit(w);
				low.set(v, Math.min(low.get(v)!, low.get(w)!));
			} else if (onStack.has(w)) {
				low.set(v, Math.min(low.get(v)!, index.get(w)!));
			}
		}
		if (low.get(v) === index.get(v)) {
			let w: string;
			do {
				w = stack.pop()!;
				onStack.delete(w);
				comp.set(w, compCount);
			} while (w !== v);
			compCount++;
		}
	};
	for (const v of ids) if (!index.has(v)) visit(v);
	return comp;
}
