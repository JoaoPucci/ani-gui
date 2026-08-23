// Acceptance: a download that ends with something to say still says
// it — visibly, on the terminal row.
//
// The backend's terminal reports (already_here, abandoned_claim,
// claim_pending) are the last line before done or error, and the dock
// used to keep them only in a hover title: nothing for touch users,
// nothing for keyboard users, and for the abandoned claim it is the
// one line carrying the path the user must delete. These scenarios
// drive the store exactly as the SSE consumer does and assert the
// translated sentence is rendered as text in the open dock, not
// tucked into an attribute.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mount, unmount, flushSync } from 'svelte';

import { downloadStore } from '../../src/lib/download/store.svelte';
import { m } from '../../src/lib/paraglide/messages';
import DownloadDock from '../../src/lib/components/DownloadDock.svelte';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
});

function openDock(): HTMLElement {
	app = mount(DownloadDock, { target });
	flushSync();
	const trigger = target.querySelector<HTMLButtonElement>('button[aria-haspopup="menu"]');
	expect(trigger, 'the dock trigger renders when items exist').not.toBeNull();
	trigger?.click();
	flushSync();
	const pop = document.getElementById('dl-dock-pop');
	expect(pop, 'the popover opens').not.toBeNull();
	return pop as HTMLElement;
}

describe('terminal download reports are visible in the dock', () => {
	it('a download that found the episode already here says so on its done row', () => {
		const id = downloadStore.add({
			title: 'Frieren',
			episode: '7',
			mode: 'sub',
			quality: '1080',
			destDir: '/dl'
		});
		downloadStore.markActive(id, new AbortController());
		downloadStore.setProgress(id, 'status.download.already_here');
		downloadStore.markDone(id, '/dl');

		const pop = openDock();
		expect(pop.textContent).toContain(m.download_status_already_here());
	});

	it('an abandoned-claim failure shows the path the user must delete', () => {
		const path = '/dl/Frieren Episode 7.mp4';
		const id = downloadStore.add({
			title: 'Frieren',
			episode: '7',
			mode: 'sub',
			quality: '1080',
			destDir: '/dl'
		});
		downloadStore.markActive(id, new AbortController());
		downloadStore.setProgress(id, `status.download.abandoned_claim ${path}`);
		downloadStore.markError(id, 'error.io');

		const pop = openDock();
		expect(pop.textContent).toContain(m.download_status_abandoned_claim({ path }));
	});
});
