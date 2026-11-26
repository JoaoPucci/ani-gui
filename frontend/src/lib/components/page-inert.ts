/**
 * The page behind a modal is `inert`: out of reach of the keyboard, the
 * pointer and assistive tech alike. `holdOutside(root)` makes everything
 * that is not `root` or one of its ancestors inert — the siblings of
 * each, up to the body — and keeps it so for what mounts beside them
 * later, a toast or a rail that loads late. `holdOutside(null)` gives
 * the page back.
 *
 * `live` elements stay within reach although they are outside `root`:
 * the place a feature tip points at, which a press may still use, and
 * the window's own buttons. Their ancestors stay out of the hold, and
 * everything else in those ancestors is held.
 *
 * It only ever lifts an `inert` it set: one the page set for its own
 * reasons stays. $lib/components/modal-layer decides which root is held.
 */

// The `inert` attributes set here.
const owned = new Set<Element>();
let held: Element[] = [];
let watcher: MutationObserver | null = null;

/** Every sibling, up to the body, of the held elements and of each of
 *  their ancestors, that holds none of them. */
function outside(kept: Element[]): Element[] {
	const out = new Set<Element>();
	const holds = (el: Element) => kept.some((k) => el === k || el.contains(k));
	for (const root of kept) {
		const body = root.ownerDocument.body;
		for (let node = root; node !== body && node.parentElement; node = node.parentElement) {
			for (const sibling of node.parentElement.children) if (!holds(sibling)) out.add(sibling);
		}
	}
	return [...out];
}

/** The containers whose children `outside` reads. */
function chain(root: Element): Element[] {
	const out: Element[] = [];
	for (let node = root.parentElement; node; node = node.parentElement) out.push(node);
	return out;
}

function apply(): void {
	const want = new Set(outside(held.filter((el) => el.isConnected)));
	for (const el of owned) {
		if (want.has(el)) continue;
		el.removeAttribute('inert');
		owned.delete(el);
	}
	for (const el of want) {
		if (el.hasAttribute('inert')) continue;
		el.setAttribute('inert', '');
		owned.add(el);
	}
}

/** Hold the page outside `root`, leaving `live` (elements outside it a
 *  press must still reach) and their ancestors within reach too. */
export function holdOutside(root: Element | null, live: readonly Element[] = []): void {
	held = root ? [root, ...live.filter((el) => el.isConnected && el !== root)] : [];
	apply();
	watcher?.disconnect();
	if (!root) return;
	watcher ??= new MutationObserver(apply);
	for (const el of new Set(held.flatMap(chain))) watcher.observe(el, { childList: true });
}
