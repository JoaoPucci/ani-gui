// Acceptance: a finished download's open-folder button opens the
// folder the episode is in. Downloads go in a folder of their own per
// show inside the folder the user chose, so that is the show's folder,
// which the backend names in the download stream's final event — not
// the chosen folder the row was started with.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mount, unmount, flushSync } from 'svelte';

import { API_BASE } from './setup';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { startDownload } from '../../src/lib/download/start';
import { downloadStore } from '../../src/lib/download/store.svelte';
import { m } from '../../src/lib/paraglide/messages';
import DownloadDock from '../../src/lib/components/DownloadDock.svelte';

import { wire } from '../answers';
import type { DownloadResponse } from '../../src/lib/api';
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

let revealed: string[] = [];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	revealed = [];
	(window as unknown as { aniGui: unknown }).aniGui = {
		revealInFolder: async (dir: string) => {
			revealed.push(dir);
			return true;
		}
	};
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
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
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

describe("the dock's open-folder button opens the show's folder", () => {
	it('opens the folder the backend says the episode landed in, not the one chosen', async () => {
		// Downloads go in a folder per show inside the chosen one: the
		// backend's final event names that folder, and the dock opens it.
		const id = startDownload({
			title: 'Re:Zero',
			episode: '2',
			mode: 'sub',
			quality: '1080',
			destDir: '/home/me/Videos'
		});
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('/api/download/stream?')),
			'the download stream'
		);
		const stream = FakeEventSource.instances.find((i) => i.url.includes('/api/download/stream?'))!;
		stream.dispatch('done', wire<DownloadResponse>({ dest_dir: '/home/me/Videos/Re - Zero' }));
		await until(
			() => downloadStore.items.find((i) => i.id === id)?.status === 'done',
			'the row to finish'
		);

		const pop = openDock();
		const open = pop.querySelector<HTMLButtonElement>(
			`button[aria-label="${m.download_dock_reveal_aria_label()}"]`
		);
		expect(open, 'a finished row offers to open its folder').not.toBeNull();
		open?.click();
		await until(() => revealed.length > 0, 'the folder to open');
		expect(revealed).toEqual(['/home/me/Videos/Re - Zero']);
	});
});
