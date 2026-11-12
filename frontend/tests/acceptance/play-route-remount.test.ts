// Acceptance: the player route mounts its page afresh for another show.
//
// The player loads the show it plays once, as it mounts. A switch to
// the show picked for a card (and a step back or forward between two
// shows' players) lands on the same route with another id, which
// SvelteKit would serve with the page it already has: the show's
// title, episodes and stream state would stay the first show's. The
// route's layout keys its page by the show, so another id is a new
// page, and another episode of the same show is not.

import { describe, it, expect, afterEach, vi } from 'vitest';
import { createRawSnippet, flushSync, mount, unmount } from 'svelte';
import { page, setParams, setUrl } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));

import PlayLayout from '../../src/routes/play/[id]/+layout.svelte';

let app: ReturnType<typeof mount> | null = null;
let target: HTMLElement;

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target?.remove();
});

describe('the player route', () => {
	it('mounts its page again for another show, and keeps it for another episode of the same', () => {
		let mounts = 0;
		const children = createRawSnippet(() => ({
			render: () => '<span data-testid="page"></span>',
			setup: () => {
				mounts++;
			}
		}));
		setParams({ id: '7158' });
		setUrl('/play/7158', { episode: '2' });
		target = document.createElement('div');
		document.body.appendChild(target);
		app = mount(PlayLayout, { target, props: { children } });
		flushSync();
		expect(mounts).toBe(1);

		setUrl('/play/7158', { episode: '3' });
		flushSync();
		expect(mounts).toBe(1);

		setParams({ id: '4013' });
		setUrl('/play/4013', { episode: '3' });
		flushSync();
		expect(mounts).toBe(2);
		expect(target.querySelectorAll('[data-testid="page"]').length).toBe(1);
	});
});
