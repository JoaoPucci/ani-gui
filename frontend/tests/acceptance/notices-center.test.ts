// Acceptance: the notification center while browsing.
//
// 1. The source stops answering: the app learns it from its own
//    traffic (the loopback health read), the bell counts one, rings,
//    and an amber toast says so. Opening the bell lists the row, links
//    it to its page, and clears the count.
// 2. The source answers again: the row says so, counts again, in a
//    neutral tone.
// 3. An update: the badge stays, the row is there with its action, and
//    the bell does not count the update a second time.
// 4. Downloads: a finished one is a row and a toast; a failed one an
//    oxblood toast that names the episode, and a row.
// 5. Under reduced motion the bell does not swing.
// 6. Rows keep their action and dismiss in place: nothing appears on
//    hover.

import { describe, it, expect, vi, afterEach } from 'vitest';

import { page, setUrl } from './page-state.svelte';
import { setReducedMotion } from './motion';
import {
	NOT_ANSWERING,
	QUIET,
	bell,
	bootLayout,
	health,
	installWindow,
	panel,
	pollAgain,
	server,
	settle,
	teardown,
	toastKinds,
	toastTexts
} from './notices-harness';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import Layout from '../../src/routes/+layout.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { downloadStore } from '../../src/lib/download/store.svelte';
import { updateStore } from '../../src/lib/update/store.svelte';

afterEach(async () => {
	await teardown();
	await setReducedMotion(false);
});

async function launch() {
	__resetApiBaseForTests('http://127.0.0.1:31337');
	setUrl('/');
	server.use(...QUIET);
	installWindow({ notification_center: true });
	return bootLayout(Layout);
}

const rows = () => Array.from(panel()?.querySelectorAll<HTMLElement>('[data-row]') ?? []);

describe('the streaming source stops answering', () => {
	it('counts it, rings, toasts amber, and lists it with a link to its page', async () => {
		const root = await launch();
		expect(bell(root)?.classList.contains('ringing')).toBe(false);
		health.answer = [NOT_ANSWERING];
		await pollAgain();
		const b = bell(root) as HTMLButtonElement;
		expect(b.getAttribute('aria-label')).toBe('Notifications, 1 unread');
		expect(b.classList.contains('tone-attention')).toBe(true);
		expect(b.classList.contains('ringing')).toBe(true);
		expect(toastTexts()).toEqual(["The streaming source isn't answering"]);
		expect(toastKinds()).toEqual(['warning']);

		b.click();
		await settle();
		const [row] = rows();
		expect(row.textContent).toContain("The streaming source isn't answering");
		expect(row.textContent).toContain('6 attempts failed');
		expect(row.querySelector('a.nr-lead')?.getAttribute('href')).toBe(
			'/notifications/outage%3Ahianime'
		);
		expect(b.getAttribute('aria-label')).toBe('Notifications');

		// Another read that still says "not answering" is no news.
		await pollAgain();
		expect(toastTexts()).toHaveLength(1);
	});

	it('says so again, in a neutral tone, once the source answers', async () => {
		const root = await launch();
		health.answer = [NOT_ANSWERING];
		await pollAgain();
		(bell(root) as HTMLButtonElement).click();
		await settle();
		(bell(root) as HTMLButtonElement).click();
		await settle();
		health.answer = [
			{ provider: 'hianime', state: 'answering_again', since_ms: 1, recovered_ms: Date.now() }
		];
		await pollAgain();
		const b = bell(root) as HTMLButtonElement;
		expect(b.getAttribute('aria-label')).toBe('Notifications, 1 unread');
		expect(b.classList.contains('tone-neutral')).toBe(true);
		expect(toastTexts()).toContain('The streaming source is answering again');
	});

	it('does not swing the bell under reduced motion', async () => {
		await setReducedMotion(true);
		const root = await launch();
		health.answer = [NOT_ANSWERING];
		await pollAgain();
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications, 1 unread');
		expect(bell(root)?.classList.contains('ringing')).toBe(false);
	});
});

describe('an update', () => {
	it('keeps its badge, is a row with its action, and is not counted twice', async () => {
		const root = await launch();
		updateStore.setAvailable({
			tag: 'v9.0.0',
			name: 'v9.0.0',
			url: 'https://github.com/JoaoPucci/ani-gui/releases/tag/v9.0.0',
			publishedAt: '',
			body: ''
		});
		await settle();
		expect(root.querySelector('.update-badge')).not.toBeNull();
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications');
		expect(toastTexts()).toEqual([]);
		(bell(root) as HTMLButtonElement).click();
		await settle();
		const [row] = rows();
		expect(row.textContent).toContain('ani-gui v9.0.0 is available');
		const action = row.querySelector<HTMLButtonElement>('.nr-action');
		expect(action?.textContent).toBe("See what's new");
		action?.click();
		await settle();
		expect(updateStore.dialogOpen).toBe(true);
		expect(panel(), 'the panel steps aside for the dialog').toBeNull();
	});
});

describe('downloads', () => {
	it('lists a finished one and toasts; a failed one toasts oxblood and names the episode', async () => {
		const root = await launch();
		const done = downloadStore.add({
			title: 'Frieren',
			episode: '1-3',
			mode: 'sub',
			quality: 'best',
			destDir: '/home/u/Videos'
		});
		downloadStore.markDone(done, '/home/u/Videos');
		const failed = downloadStore.add({
			title: 'Frieren',
			episode: '4',
			mode: 'sub',
			quality: 'best',
			destDir: '/home/u/Videos'
		});
		downloadStore.markError(failed, 'The source stopped answering.');
		await settle();
		const toasts = toastTexts().map((t, i) => [t, toastKinds()[i]]);
		expect(toasts).toHaveLength(2);
		expect(toasts).toContainEqual(['3 episodes downloaded', 'info']);
		expect(toasts).toContainEqual(['A download failed · Frieren · Episode 4', 'error']);
		expect(bell(root)?.getAttribute('aria-label')).toBe('Notifications, 2 unread');
		(bell(root) as HTMLButtonElement).click();
		await settle();
		const byText = (t: string) => rows().find((r) => r.textContent?.includes(t));
		expect(byText('A download failed')).toBeDefined();
		expect(byText('3 episodes downloaded')?.querySelector('.nr-action')?.textContent).toBe(
			'Show in folder'
		);
	});

	it('keeps every row control in place: action and dismiss are there without a hover', async () => {
		const root = await launch();
		const id = downloadStore.add({
			title: 'Frieren',
			episode: '4',
			mode: 'sub',
			quality: 'best',
			destDir: '/v'
		});
		downloadStore.markDone(id, '/v');
		await settle();
		(bell(root) as HTMLButtonElement).click();
		await settle();
		const [row] = rows();
		expect(row.querySelector('.nr-end .nr-dismiss')).not.toBeNull();
		expect(row.querySelector('.nr-foot .nr-action')).not.toBeNull();
		(row.querySelector('.nr-dismiss') as HTMLButtonElement).click();
		await settle();
		expect(rows()).toHaveLength(0);
		expect(panel()?.textContent).toContain("You're all caught up");
	});
});

describe('keys in the panel off the player', () => {
	it("still reach the app's own shortcuts, such as / for search", async () => {
		const root = await launch();
		const id = downloadStore.add({
			title: 'Frieren',
			episode: '4',
			mode: 'sub',
			quality: 'best',
			destDir: '/v'
		});
		downloadStore.markDone(id, '/v');
		await settle();
		(bell(root) as HTMLButtonElement).click();
		await settle();
		const seen: string[] = [];
		const spy = (e: KeyboardEvent) => seen.push(e.key);
		window.addEventListener('keydown', spy);
		try {
			const lead = panel()?.querySelector<HTMLElement>('.nr-lead') as HTMLElement;
			lead.dispatchEvent(new KeyboardEvent('keydown', { key: '/', bubbles: true }));
			expect(seen).toEqual(['/']);
		} finally {
			window.removeEventListener('keydown', spy);
		}
	});
});
