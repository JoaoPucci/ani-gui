/**
 * The page behind a modal is `inert`: out of reach of the keyboard, the
 * pointer and assistive tech alike. `holdOutside(root)` makes everything
 * that is not `root` or one of its ancestors inert — the siblings of
 * each, up to the body — and keeps it so for what mounts beside them
 * later, a toast or a rail that loads late. `holdOutside(null)` gives
 * the page back.
 *
 * It only ever lifts an `inert` it set: one the page set for its own
 * reasons stays. $lib/components/modal-layer decides which root is held.
 */

// The `inert` attributes set here.
const owned = new Set<Element>();
let held: Element | null = null;
let watcher: MutationObserver | null = null;

function outside(root: Element): Element[] {
	const out: Element[] = [];
	const body = root.ownerDocument.body;
	for (let node = root; node !== body && node.parentElement; node = node.parentElement) {
		for (const sibling of node.parentElement.children) if (sibling !== node) out.push(sibling);
	}
	return out;
}

/** The containers whose children `outside` reads. */
function chain(root: Element): Element[] {
	const out: Element[] = [];
	for (let node = root.parentElement; node; node = node.parentElement) out.push(node);
	return out;
}

function apply(): void {
	const want = new Set(held ? outside(held) : []);
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

export function holdOutside(root: Element | null): void {
	held = root;
	apply();
	watcher?.disconnect();
	if (!root) return;
	watcher ??= new MutationObserver(apply);
	for (const el of chain(root)) watcher.observe(el, { childList: true });
}
