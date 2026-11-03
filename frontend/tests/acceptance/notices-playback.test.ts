// Acceptance: the notification center while an episode plays.
//
// 1. While a video plays, only a failure raises a toast: the source
//    not answering is counted on the bell and listed, silently.
// 2. Nothing ever shows over fullscreen. A failure that happens in
//    fullscreen is counted at once, and its toast shows when the user
//    leaves fullscreen.
// 3. On the player nothing navigates away from the episode: a toast's
//    View and a row open the notice in place, in the panel.

import { describe, it, expect, vi, afterEach } from 'vitest';

import { page, setUrl } from './page-state.svelte';
import {
	NOT_ANSWERING,
	QUIET,
	bell,
	bootLayout,
	health,
	installWindow,
	panel,
	playingVideo,
	pollAgain,
	server,
	setFullscreen,
	settle,
	teardown,
	toastTexts
} from './notices-harness';

const goto = vi.fn(async () => {});
vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: (...args: unknown[]) => goto(...(args as [])),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import Layout from '../../src/routes/+layout.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { downloadStore } from '../../src/lib/download/store.svelte';
import { toastStore } from '../../src/lib/toasts/store.svelte';

let video: (HTMLVideoElement & { stop(): void }) | null = null;

afterEach(async () => {
	video?.remove();
	video = null;
	goto.mockClear();
	await teardown();
});

async function launchOnPlayer() {
	__resetApiBaseForTests('http://127.0.0.1:31337');
	setUrl('/play/123');
	server.use(...QUIET);
	installWindow({ notification_center: true });
	const root = await bootLayout(Layout);
	video = playingVideo();
	await settle();
	return root;
}

function failADownload() {
	const id = downloadStore.add({
		title: 'Frieren',
		episode: '7',
		mode: 'sub',
		quality: 'best',
		destDir: '/v'
	});
	downloadStore.markError(id, 'The source stopped answering.');
}

describe('while an episode plays', () => {
	it('toasts only a failure; the source not answering is counted silently', async () => {
		const root = await launchOnPlayer();
		health.answer = [NOT_ANSWERING];
		await pollAgain();
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications, 1 unread');
		expect(toastTexts()).toEqual([]);
		failADownload();
		await settle();
		expect(toastTexts()).toEqual(['A download failed · Frieren · Episode 7']);
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications, 2 unread');
	});

	it('shows nothing over fullscreen, counts at once, and toasts on leaving it', async () => {
		const root = await launchOnPlayer();
		setFullscreen(document.createElement('div'));
		await settle();
		failADownload();
		await settle();
		expect(toastTexts(), 'nothing over fullscreen').toEqual([]);
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications, 1 unread');
		setFullscreen(null);
		await settle();
		expect(toastTexts()).toEqual(['A download failed · Frieren · Episode 7']);
	});

	it('opens a notice in place, from its toast or its row, never navigating away', async () => {
		const root = await launchOnPlayer();
		failADownload();
		await settle();
		const toast = toastStore.items[0];
		toast.onAction?.();
		await settle();
		expect(goto).not.toHaveBeenCalled();
		const p = panel() as HTMLElement;
		expect(p).not.toBeNull();
		const lead = p.querySelector<HTMLButtonElement>('button.nr-lead');
		expect(lead?.getAttribute('aria-expanded')).toBe('true');
		expect(p.textContent).toContain('The source stopped answering.');
		expect(p.querySelector('a.nr-lead')).toBeNull();
		lead?.click();
		await settle();
		expect(lead?.getAttribute('aria-expanded')).toBe('false');
		expect(goto).not.toHaveBeenCalled();
		expect(bell(root)).not.toBeNull();
	});
});

describe('once the episode stops', () => {
	it('toasts news again', async () => {
		await launchOnPlayer();
		video?.stop();
		await settle();
		health.answer = [NOT_ANSWERING];
		await pollAgain();
		expect(toastTexts()).toEqual(["The streaming source isn't answering"]);
	});
});
