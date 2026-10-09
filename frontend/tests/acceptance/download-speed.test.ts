// Acceptance: the download indicators show how fast downloads are
// going. The backend reports each running download's speed once a
// second as a status line; these drive the store as the SSE consumer
// does and assert the translated speed is rendered as text beside the
// top bar's download indicator and in the bottom bar.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mount, unmount, flushSync } from 'svelte';

import { downloadStore } from '../../src/lib/download/store.svelte';
import { m } from '../../src/lib/paraglide/messages';
import DownloadDock from '../../src/lib/components/DownloadDock.svelte';
import DownloadBar from '../../src/lib/components/DownloadBar.svelte';

let target: HTMLElement;
let apps: ReturnType<typeof mount>[] = [];

beforeEach(() => {
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	for (const app of apps) unmount(app);
	apps = [];
	target.remove();
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
});

function startDownload(title: string, bytesPerSecond: number): string {
	const id = downloadStore.add({
		title,
		episode: '7',
		mode: 'sub',
		quality: '1080',
		destDir: '/dl'
	});
	downloadStore.markActive(id, new AbortController());
	downloadStore.setProgress(id, `status.download.rate ${bytesPerSecond}`);
	return id;
}

describe('download speed', () => {
	it('shows the speed beside the top bar indicator and in the bottom bar', () => {
		startDownload('Frieren', 1.5 * 1024 * 1024);
		apps.push(mount(DownloadDock, { target }));
		apps.push(mount(DownloadBar, { target }));
		flushSync();
		const expected = m.download_speed_mbps({ value: '1.5' });
		const trigger = target.querySelector('button[aria-haspopup="menu"]');
		expect(trigger?.textContent).toContain(expected);
		expect(target.querySelector('.dl-bar')?.textContent).toContain(expected);
	});

	it("names the speed in the top bar indicator's accessible label", () => {
		// The indicator's label is what a screen reader announces for it,
		// and it replaces the button's text: the speed has to be in it.
		startDownload('Frieren', 1.5 * 1024 * 1024);
		apps.push(mount(DownloadDock, { target }));
		flushSync();
		const trigger = target.querySelector('button[aria-haspopup="menu"]');
		expect(trigger?.getAttribute('aria-label')).toBe(
			m.download_dock_active_speed_label({
				count: 1,
				speed: m.download_speed_mbps({ value: '1.5' })
			})
		);
	});

	it('adds up every running download', () => {
		startDownload('Frieren', 300 * 1024);
		startDownload('Dandadan', 200 * 1024);
		apps.push(mount(DownloadBar, { target }));
		flushSync();
		expect(target.querySelector('.dl-bar')?.textContent).toContain(
			m.download_speed_kbps({ value: '500' })
		);
	});

	it('shows zero as soon as a download is running, before its first bytes', () => {
		// A download waiting on its first bytes is still running; a
		// readout that stayed hidden until then read as no change.
		const id = downloadStore.add({
			title: 'Frieren',
			episode: '7',
			mode: 'sub',
			quality: '1080',
			destDir: '/dl'
		});
		downloadStore.markActive(id, new AbortController());
		apps.push(mount(DownloadDock, { target }));
		apps.push(mount(DownloadBar, { target }));
		flushSync();
		const zero = m.download_speed_kbps({ value: '0' });
		expect(target.querySelector('.dl-bar-speed')?.textContent).toContain(zero);
		const trigger = target.querySelector('button[aria-haspopup="menu"]');
		expect(trigger?.textContent).toContain(zero);
		expect(trigger?.getAttribute('aria-label')).toBe(
			m.download_dock_active_speed_label({ count: 1, speed: zero })
		);
	});
});
