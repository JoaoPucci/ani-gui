// Acceptance: a download past the ones running at once waits in the
// dock as queued, and starts when one of them ends.
//
// A download reports its progress over a stream of its own, open for
// the whole download, and downloads share the six connections Chromium
// opens to their lane. A seventh used to wait for one of them inside
// Chromium, invisibly, while the dock showed it as started; the
// backend does not start a download until its stream arrives. Now the
// renderer starts at most as many as there are connections and shows
// the rest as queued, cancellable like any other row.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mount, unmount, flushSync } from 'svelte';

import { downloadStore } from '../../src/lib/download/store.svelte';
import { startDownload } from '../../src/lib/download/start';
import { DOWNLOAD_CONCURRENCY } from '../../src/lib/download/queue';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';
import DownloadDock from '../../src/lib/components/DownloadDock.svelte';
import DownloadBar from '../../src/lib/components/DownloadBar.svelte';
import { formatSpeed } from '../../src/lib/download/speed';
import { API_BASE } from './setup';

type Handler = (ev: MessageEvent) => void;
class FakeEventSource {
	static open: FakeEventSource[] = [];
	static opened = 0;
	url: string;
	listeners: Record<string, Handler[]> = {};
	constructor(url: string) {
		this.url = url;
		FakeEventSource.open.push(this);
		FakeEventSource.opened += 1;
	}
	addEventListener(name: string, h: Handler) {
		(this.listeners[name] ??= []).push(h);
	}
	close() {
		FakeEventSource.open = FakeEventSource.open.filter((e) => e !== this);
	}
	finish() {
		for (const h of this.listeners.done ?? [])
			h({ data: JSON.stringify({ dest_dir: '/dl' }) } as MessageEvent);
	}
}

const g = globalThis as unknown as { EventSource?: unknown };
let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let bar: ReturnType<typeof mount> | null = null;

/** Messages by name, so a case can name one the catalogue lacks and
 *  fail on it rather than on the compile. */
const message = (name: string) =>
	(m as unknown as Record<string, ((inputs: Record<string, unknown>) => string) | undefined>)[name];

const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	g.EventSource = FakeEventSource;
	FakeEventSource.open = [];
	FakeEventSource.opened = 0;
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(async () => {
	for (const item of [...downloadStore.items]) downloadStore.cancel(item.id);
	await settle();
	if (app) unmount(app);
	app = null;
	if (bar) unmount(bar);
	bar = null;
	target.remove();
	delete g.EventSource;
});

function startEpisodes(count: number): string[] {
	return Array.from({ length: count }, (_, i) =>
		startDownload({
			title: 'Frieren',
			episode: String(i + 1),
			mode: 'sub',
			quality: '1080',
			destDir: '/dl'
		})
	);
}

function openDock(): HTMLElement {
	app = mount(DownloadDock, { target });
	flushSync();
	target.querySelector<HTMLButtonElement>('button[aria-haspopup="menu"]')?.click();
	flushSync();
	return document.getElementById('dl-dock-pop') as HTMLElement;
}

function rowFor(pop: HTMLElement, episode: number): HTMLElement {
	const label = m.download_dock_ep_label_single({ episode: String(episode) });
	const row = Array.from(pop.querySelectorAll<HTMLElement>('li')).find(
		(li) => li.querySelector('.dl-row-ep')?.textContent === label
	);
	expect(row, `the row for episode ${episode}`).toBeDefined();
	return row!;
}

describe('downloads past the ones running at once', () => {
	it('wait as queued, cancellable, and start when a running one ends', async () => {
		startEpisodes(DOWNLOAD_CONCURRENCY + 1);
		await settle();
		expect(FakeEventSource.opened).toBe(DOWNLOAD_CONCURRENCY);

		const pop = openDock();
		const waiting = rowFor(pop, DOWNLOAD_CONCURRENCY + 1);
		expect(waiting.textContent).toContain(m.download_dock_queued());
		expect(
			waiting.querySelector(`button[aria-label="${m.download_dock_cancel_aria_label()}"]`)
		).not.toBeNull();
		expect(rowFor(pop, 1).textContent).not.toContain(m.download_dock_queued());

		// What is running and what is waiting are told apart: the
		// trigger's label and the progress strip count each.
		const trigger = target.querySelector<HTMLButtonElement>('button[aria-haspopup="menu"]');
		expect(trigger?.getAttribute('aria-label')).toBe(
			message('download_dock_active_queued_speed_label')?.({
				count: DOWNLOAD_CONCURRENCY,
				queued: 1,
				speed: formatSpeed(0)
			})
		);
		bar = mount(DownloadBar, { target });
		flushSync();
		expect(target.querySelector('.dl-bar')?.textContent).toContain(
			message('download_bar_label_multi_queued')?.({ count: DOWNLOAD_CONCURRENCY, queued: 1 }) ??
				'the strip names the queued downloads'
		);

		FakeEventSource.open[0].finish();
		await settle();
		flushSync();
		expect(FakeEventSource.opened).toBe(DOWNLOAD_CONCURRENCY + 1);
		expect(rowFor(pop, DOWNLOAD_CONCURRENCY + 1).textContent).not.toContain(
			m.download_dock_queued()
		);
	});

	it('a queued download cancelled before its turn never starts', async () => {
		const ids = startEpisodes(DOWNLOAD_CONCURRENCY + 1);
		await settle();
		downloadStore.cancel(ids[DOWNLOAD_CONCURRENCY]);
		await settle();
		FakeEventSource.open[0].finish();
		await settle();
		expect(FakeEventSource.opened).toBe(DOWNLOAD_CONCURRENCY);
		expect(downloadStore.items.some((i) => i.id === ids[DOWNLOAD_CONCURRENCY])).toBe(false);
	});
});
