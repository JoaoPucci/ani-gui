// Acceptance: a download of an episode the source does not carry
// tells the user so, in the dock, with the play page's own words.
//
// The backend ends the download stream with the typed episode
// verdict — a unit variant, so no message rides with it — and the
// glue between the stream, the store and the dock used to let it
// fall through to the generic "Download failed". This scenario
// starts a real download through the real glue over a faked
// EventSource, fails it with the verdict as the sidecar sends it,
// and reads the rendered dock row, so a dropped call anywhere
// between `startDownload`, the store and the dock's error marker
// cannot hide behind the mocked unit case.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mount, unmount, flushSync } from 'svelte';

import { API_BASE } from './setup';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { startDownload } from '../../src/lib/download/start';
import { downloadStore } from '../../src/lib/download/store.svelte';
import { m } from '../../src/lib/paraglide/messages';
import DownloadDock from '../../src/lib/components/DownloadDock.svelte';

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	closed = false;
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {
		this.closed = true;
	}
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	for (const item of [...downloadStore.items]) downloadStore.dismiss(item.id);
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

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

describe('the download dock renders the episode verdict', () => {
	it('an episode the source does not carry names the episode on its failed row', async () => {
		const id = startDownload({
			title: 'Frieren',
			episode: '7',
			mode: 'sub',
			quality: '1080',
			destDir: '/dl'
		});
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('/api/download/stream?')),
			'the download stream'
		);
		const stream = FakeEventSource.instances.find((i) => i.url.includes('/api/download/stream?'))!;
		// What the sidecar sends when the show is there and this
		// episode is not: the typed verdict with its key, no message.
		stream.dispatch(
			'error',
			JSON.stringify({ kind: 'episode_unavailable', key: 'error.play.episode_unavailable' })
		);
		await until(
			() => downloadStore.items.find((i) => i.id === id)?.status === 'error',
			'the row to fail'
		);

		const pop = openDock();
		// The failed row's own class is dl-row-error too; the marker is
		// the span inside it.
		const marker = pop.querySelector<HTMLElement>('span.dl-row-error');
		expect(marker, 'the failed row carries its error marker').not.toBeNull();
		expect(marker?.getAttribute('title')).toBe(m.play_play_failure_episode_unavailable());
		expect(marker?.getAttribute('title')).not.toBe(m.errors_failed_default());
		expect(pop.textContent).toContain('Frieren');
	});
});
