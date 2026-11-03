// What the notification-center scenarios share: a layout booted with
// the launch flags a scenario chooses, a backend that answers the
// center's one loopback read, and a clean slate between scenarios.
//
// The center is the window's single instance, so a scenario's rows,
// open panel and held toasts would otherwise reach the next one.

import { http, HttpResponse } from 'msw';
import { mount, unmount, createRawSnippet } from 'svelte';

import { API_BASE, server } from './setup';
import { IDLE } from '../../src/lib/notices/arrival';
import { noticeCenter } from '../../src/lib/notices/instance';
import { STORAGE_KEY } from '../../src/lib/notices/model';
import { downloadStore } from '../../src/lib/download/store.svelte';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { updateStore } from '../../src/lib/update/store.svelte';

export type Health =
	| { provider: string; state: 'answering' }
	| {
			provider: string;
			state: 'not_answering';
			since_ms: number;
			failures: number;
			last_failure_ms: number;
	  }
	| { provider: string; state: 'answering_again'; since_ms: number; recovered_ms: number };

/** What `/api/source-health` answers; a scenario changes it between
 *  reads. Counts the reads. */
export const health = { answer: [] as Health[], reads: 0 };

export const QUIET = [
	http.get(`${API_BASE}/api/update-check`, () => HttpResponse.json(null, { status: 503 })),
	http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(null, { status: 503 })),
	http.get(`${API_BASE}/api/source-health`, () => {
		health.reads += 1;
		return HttpResponse.json({ providers: health.answer });
	})
];

export function installWindow(flags: Record<string, boolean | string> | undefined) {
	(window as unknown as { aniGui: unknown }).aniGui = {
		apiBase: API_BASE,
		flags,
		account: { getToken: () => ({ ok: false, kind: 'not_found' }) }
	};
}

let target: HTMLElement | null = null;
let layout: ReturnType<typeof mount> | null = null;

export async function bootLayout(Layout: Parameters<typeof mount>[0]) {
	target = document.createElement('div');
	document.body.appendChild(target);
	const children = createRawSnippet(() => ({ render: () => '<div></div>' }));
	layout = mount(Layout, { target, props: { children } });
	await settle();
	return target;
}

export async function settle(steps = 30) {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 5));
}

export function resetCenter() {
	noticeCenter.rows = [];
	noticeCenter.open = false;
	noticeCenter.expanded = null;
	noticeCenter.setMoment(IDLE);
}

export async function teardown() {
	await settle(2);
	if (layout) unmount(layout);
	layout = null;
	target?.remove();
	target = null;
	for (const t of [...toastStore.items]) toastStore.dismiss(t.id);
	for (const d of [...downloadStore.items]) downloadStore.dismiss(d.id);
	updateStore.setAvailable(null);
	updateStore.closeDialog();
	resetCenter();
	health.answer = [];
	health.reads = 0;
	window.localStorage.removeItem(STORAGE_KEY);
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
	setFullscreen(null);
}

export const bell = (root: ParentNode) => root.querySelector<HTMLButtonElement>('.nc-bell');
export const panel = () => document.querySelector<HTMLElement>('#nc-panel');
export const toastTexts = (root: ParentNode = document) =>
	Array.from(root.querySelectorAll('.toast .message')).map((el) => el.textContent?.trim() ?? '');
export const toastKinds = (root: ParentNode = document) =>
	Array.from(root.querySelectorAll<HTMLElement>('.toast')).map((el) => el.dataset.kind);

/** Makes `el` (or nothing) the fullscreen element and tells the
 *  document, as Chromium does on entering or leaving fullscreen. */
export function setFullscreen(el: Element | null) {
	Object.defineProperty(document, 'fullscreenElement', { configurable: true, get: () => el });
	document.dispatchEvent(new Event('fullscreenchange'));
}

/** A video in the page that is playing, as the player's is. */
export function playingVideo(): HTMLVideoElement & { stop(): void } {
	const v = document.createElement('video');
	let paused = false;
	Object.defineProperty(v, 'paused', { configurable: true, get: () => paused });
	document.body.appendChild(v);
	v.dispatchEvent(new Event('play'));
	return Object.assign(v, {
		stop() {
			paused = true;
			v.dispatchEvent(new Event('pause'));
		}
	});
}

export const NOT_ANSWERING: Health = {
	provider: 'hianime',
	state: 'not_answering',
	since_ms: Date.now() - 600_000,
	failures: 6,
	last_failure_ms: Date.now() - 60_000
};

/** Ask the center to read the health again, as a window focus does. */
export async function pollAgain() {
	window.dispatchEvent(new Event('focus'));
	await settle();
}

export { server };
