// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { isModalOpen, modal, openModal, type ModalHandle } from './modal-layer';

// A page shaped like the app's: a header beside the content, the
// content holding a section and the modal's mount point.
let page: HTMLElement;
let header: HTMLElement;
let section: HTMLElement;
let opener: HTMLButtonElement;
let open: ModalHandle[] = [];

function dialog(html = '<button id="first">first</button><button id="last">last</button>') {
	const el = document.createElement('div');
	el.tabIndex = -1;
	el.innerHTML = html;
	return el;
}
function mountIn(parent: HTMLElement, el: HTMLElement, opts = {}) {
	parent.appendChild(el);
	const handle = openModal(el, opts);
	open.push(handle);
	return handle;
}
const inert = (el: Element) => el.closest('[inert]') !== null;
function press(key: string, shiftKey = false) {
	const at = (document.activeElement ?? document.body) as HTMLElement;
	const ev = new KeyboardEvent('keydown', { key, shiftKey, bubbles: true, cancelable: true });
	at.dispatchEvent(ev);
	return ev;
}
const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
	document.body.innerHTML = `
		<header id="header"><button id="badge">badge</button></header>
		<main id="page"><section id="section"><button id="opener">open</button></section></main>`;
	page = document.getElementById('page')!;
	header = document.getElementById('header')!;
	section = document.getElementById('section')!;
	opener = document.getElementById('opener') as HTMLButtonElement;
	opener.focus();
	open = [];
});

afterEach(() => {
	for (const h of open) h.close();
	document.body.innerHTML = '';
});

describe('a modal', () => {
	it('takes focus at its first control', () => {
		const d = dialog();
		mountIn(page, d);
		expect(document.activeElement?.id).toBe('first');
	});

	it('takes focus at the control marked data-autofocus', () => {
		const d = dialog('<button>cancel</button><button id="ok" data-autofocus>ok</button>');
		mountIn(page, d);
		expect(document.activeElement?.id).toBe('ok');
	});

	it('takes focus on itself when nothing in it does', () => {
		const d = dialog('<p>working</p>');
		mountIn(page, d);
		expect(document.activeElement).toBe(d);
	});

	it('makes the rest of the page inert, and not itself', () => {
		const d = dialog();
		mountIn(page, d);
		expect(inert(header)).toBe(true);
		expect(inert(section)).toBe(true);
		expect(inert(opener)).toBe(true);
		expect(inert(d)).toBe(false);
		expect(page.hasAttribute('inert')).toBe(false);
	});

	it('makes inert what mounts beside it while it is up', async () => {
		const d = dialog();
		mountIn(page, d);
		const late = document.createElement('section');
		page.appendChild(late);
		const toast = document.createElement('div');
		document.body.appendChild(toast);
		await flush();
		expect(inert(late)).toBe(true);
		expect(inert(toast)).toBe(true);
	});

	it('keeps Tab and Shift+Tab inside it, wrapping at the ends', () => {
		mountIn(page, dialog());
		expect(press('Tab').defaultPrevented).toBe(true);
		expect(document.activeElement?.id).toBe('last');
		press('Tab');
		expect(document.activeElement?.id).toBe('first');
		press('Tab', true);
		expect(document.activeElement?.id).toBe('last');
	});

	it('holds Tab on itself when nothing in it takes focus', () => {
		const d = dialog('<p>working</p>');
		mountIn(page, d);
		expect(press('Tab').defaultPrevented).toBe(true);
		expect(document.activeElement).toBe(d);
	});

	it('answers Escape with its close, and only then', () => {
		const onEscape = vi.fn();
		const behind = vi.fn();
		window.addEventListener('keydown', behind);
		mountIn(page, dialog(), { onEscape });
		const ev = press('Escape');
		expect(onEscape).toHaveBeenCalledTimes(1);
		expect(ev.defaultPrevented).toBe(true);
		// Nothing behind the modal hears it.
		expect(behind).not.toHaveBeenCalled();
		window.removeEventListener('keydown', behind);
	});

	it('leaves Escape alone without a close, and other keys always', () => {
		mountIn(page, dialog());
		expect(press('Escape').defaultPrevented).toBe(false);
		expect(press('Enter').defaultPrevented).toBe(false);
	});

	it('reads the close it is given last', () => {
		const first = vi.fn();
		const second = vi.fn();
		const h = mountIn(page, dialog(), { onEscape: first });
		h.update({ onEscape: second });
		press('Escape');
		expect(first).not.toHaveBeenCalled();
		expect(second).toHaveBeenCalledTimes(1);
	});

	it('says the page is held while it is up, and only then', () => {
		expect(isModalOpen()).toBe(false);
		const h = mountIn(page, dialog());
		expect(isModalOpen()).toBe(true);
		h.close();
		expect(isModalOpen()).toBe(false);
	});

	it('gives the page and focus back as it closes', () => {
		const h = mountIn(page, dialog());
		h.close();
		expect(inert(header)).toBe(false);
		expect(inert(opener)).toBe(false);
		expect(document.activeElement).toBe(opener);
		// The keyboard is the page's again.
		expect(press('Tab').defaultPrevented).toBe(false);
	});

	it('closes once: a second close does nothing', () => {
		const h = mountIn(page, dialog());
		h.close();
		document.getElementById('badge')!.focus();
		h.close();
		expect(document.activeElement?.id).toBe('badge');
	});

	it('leaves focus where it is when its opener has gone', () => {
		const h = mountIn(page, dialog());
		opener.remove();
		h.close();
		expect(document.activeElement).not.toBe(opener);
	});

	it('has nothing to give back when nothing held focus as it opened', () => {
		(document.activeElement as HTMLElement | null)?.blur();
		const h = mountIn(page, dialog());
		h.close();
		// Focus is not moved anywhere: it stays where the modal put it.
		expect(document.activeElement?.id).toBe('first');
	});

	it('never lifts an inert the page set itself', () => {
		header.setAttribute('inert', '');
		const h = mountIn(page, dialog());
		h.close();
		expect(header.hasAttribute('inert')).toBe(true);
	});

	it('works as a Svelte action', () => {
		const onEscape = vi.fn();
		const d = dialog();
		page.appendChild(d);
		const action = modal(d, { onEscape });
		expect(document.activeElement?.id).toBe('first');
		action.update({ onEscape });
		press('Escape');
		expect(onEscape).toHaveBeenCalledTimes(1);
		action.destroy();
		expect(document.activeElement).toBe(opener);
	});

	it('takes no options as a Svelte action, too', () => {
		const d = dialog();
		page.appendChild(d);
		const action = modal(d);
		expect(press('Escape').defaultPrevented).toBe(false);
		action.destroy();
	});
});

describe('modals on top of each other', () => {
	it('lets only the topmost hold the keyboard, and hands back to the one beneath', () => {
		const lowerEscape = vi.fn();
		const lower = dialog('<button id="l1">l1</button><button id="l2">l2</button>');
		mountIn(page, lower, { onEscape: lowerEscape });
		const upper = dialog('<button id="u1">u1</button>');
		const top = mountIn(document.body, upper);
		expect(document.activeElement?.id).toBe('u1');
		expect(inert(lower)).toBe(true);
		press('Tab');
		expect(document.activeElement?.id).toBe('u1');
		press('Escape');
		expect(lowerEscape).not.toHaveBeenCalled();

		top.close();
		expect(inert(lower)).toBe(false);
		expect(inert(header)).toBe(true);
		expect(document.activeElement?.id).toBe('l1');
		press('Escape');
		expect(lowerEscape).toHaveBeenCalledTimes(1);
	});

	it('passes the return target up when the one beneath closes first', () => {
		// A resume's loading overlay, then its failure over it, then the
		// loading overlay going.
		const loading = dialog('<p>loading</p>');
		const below = mountIn(page, loading);
		expect(document.activeElement).toBe(loading);
		const failure = dialog('<button id="dismiss">ok</button>');
		const above = mountIn(page, failure);
		below.close();
		loading.remove();
		// Focus stays in the failure, and the page stays held.
		expect(document.activeElement?.id).toBe('dismiss');
		expect(inert(opener)).toBe(true);
		above.close();
		expect(document.activeElement).toBe(opener);
	});

	it('passes up only the return targets that were inside the one closing', () => {
		const a = mountIn(page, dialog('<button id="a1">a1</button>'));
		const b = mountIn(page, dialog('<button id="b1">b1</button>'));
		const c = mountIn(page, dialog('<button id="c1">c1</button>'));
		a.close();
		c.close();
		expect(document.activeElement?.id).toBe('b1');
		b.close();
		expect(document.activeElement).toBe(opener);
	});

	it('returns focus into the one beneath when the upper opened with focus nowhere', () => {
		// A busy confirm's disabled button drops focus to the body; a
		// modal that opens then has no opener of its own.
		const lower = dialog('<button id="l1">l1</button>');
		const below = mountIn(page, lower);
		(document.activeElement as HTMLElement).blur();
		const above = mountIn(page, dialog('<button id="u1">u1</button>'));
		below.close();
		above.close();
		expect(document.activeElement).toBe(opener);
	});

	it('enters the one beneath at its control when the upper, opened with focus nowhere, closes first', () => {
		// The lower one's own surface does not take focus (the update
		// dialog's backdrop has no tabindex), so focus goes to its control.
		const lower = document.createElement('div');
		lower.innerHTML = '<button id="l1">l1</button><button id="l2">l2</button>';
		page.appendChild(lower);
		open.push(openModal(lower));
		(document.activeElement as HTMLElement).blur();
		const above = mountIn(page, dialog('<button id="u1">u1</button>'));
		above.close();
		expect(document.activeElement?.id).toBe('l1');
	});

	it('enters the one beneath when the closing one’s opener has gone', () => {
		const lower = dialog('<button id="l1">l1</button>');
		mountIn(page, lower);
		const top = mountIn(page, dialog('<button id="u1">u1</button>'));
		document.getElementById('l1')!.remove();
		top.close();
		expect(document.activeElement).toBe(lower);
	});
});

describe('a modal opening over a fullscreen element', () => {
	afterEach(() => {
		Reflect.deleteProperty(document, 'fullscreenElement');
		Reflect.deleteProperty(document, 'exitFullscreen');
	});
	function fullscreen(el: Element) {
		const exit = vi.fn(async () => {});
		Object.defineProperty(document, 'fullscreenElement', { value: el, configurable: true });
		Object.defineProperty(document, 'exitFullscreen', { value: exit, configurable: true });
		return exit;
	}

	it('leaves fullscreen when it would open hidden behind it', () => {
		const exit = fullscreen(section);
		mountIn(page, dialog());
		expect(exit).toHaveBeenCalledTimes(1);
	});

	it('stays in fullscreen when it opens inside the fullscreen element', () => {
		const exit = fullscreen(section);
		mountIn(section, dialog());
		expect(exit).not.toHaveBeenCalled();
	});

	it('stays in fullscreen for a native dialog, which the page shows in the top layer', () => {
		const exit = fullscreen(section);
		mountIn(page, document.createElement('dialog'));
		expect(exit).not.toHaveBeenCalled();
	});

	it('shrugs off a refused exit', async () => {
		const exit = fullscreen(section);
		exit.mockImplementation(async () => {
			throw new Error('refused');
		});
		mountIn(page, dialog());
		await flush();
		expect(exit).toHaveBeenCalledTimes(1);
	});
});

describe('a modal opening over a native modal dialog', () => {
	// A native <dialog> shown with showModal() sits in the top layer: a
	// modal of the layer opening above it would be painted beneath it and,
	// outside the dialog, held inert by the browser. The dialog steps out
	// of the top layer while the modal above it is up, and comes back,
	// with focus inside it, once that modal closes.
	function nativeDialog() {
		const el = document.createElement('dialog');
		el.innerHTML = '<input id="search" /><button id="pick">pick</button>';
		page.appendChild(el);
		el.showModal();
		open.push(openModal(el));
		return el;
	}

	it('steps the dialog out of the top layer while the modal above is up', () => {
		const native = nativeDialog();
		const above = mountIn(page, dialog());
		expect(native.open).toBe(false);
		void above;
	});

	it('leaves a dialog gone from the page closed', () => {
		const native = nativeDialog();
		const above = mountIn(page, dialog());
		native.remove();
		const showModal = vi.spyOn(native, 'showModal');
		above.close();
		expect(showModal).not.toHaveBeenCalled();
	});

	it('brings it back, focus inside it, once the modal above closes', () => {
		const native = nativeDialog();
		const above = mountIn(page, dialog());
		above.close();
		expect(native.open).toBe(true);
		expect(native.contains(document.activeElement)).toBe(true);
	});
});
