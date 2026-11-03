// Acceptance: a notice's own page, and the line above Play.
//
// 1. A notice's page shows its title, its time and everything it has
//    to say; the outage page says plainly that the cause is not known
//    and what the app saw, from its own traffic only.
// 2. A notice no longer in the list, or any notice with the center
//    off, says it is no longer here.
// 3. While the source is not answering, a show's page carries one line
//    above Play that says so and links to the notice; with the center
//    off, or the source answering, nothing.

import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { mount, unmount } from 'svelte';

import { page, setParams } from './page-state.svelte';
import { installWindow, resetCenter } from './notices-harness';

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

import DetailPage from '../../src/routes/notifications/[key]/+page.svelte';
import OutageLine from '../../src/lib/notices/OutageLine.svelte';
import { noticeCenter } from '../../src/lib/notices/instance';
import { updateStore } from '../../src/lib/update/store.svelte';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	resetCenter();
	updateStore.closeDialog();
	(window as unknown as { aniGui?: unknown }).aniGui = undefined;
});

const tick = () => new Promise((r) => setTimeout(r, 10));

function raiseOutage() {
	noticeCenter.raise({
		key: 'outage:hianime',
		kind: 'outage',
		data: { since: Date.now() - 3600_000, failures: 5, last: Date.now() - 60_000 }
	});
}

describe("a notice's page", () => {
	it('says what the outage is, that its cause is unknown, and what was seen', async () => {
		installWindow({ notification_center: true });
		raiseOutage();
		setParams({ key: 'outage:hianime' });
		app = mount(DetailPage, { target });
		await tick();
		expect(target.querySelector('h1')?.textContent).toBe("The streaming source isn't answering");
		expect(target.textContent).toContain("We don't know why yet");
		expect(target.textContent).toContain('What ani-gui saw');
		expect(target.textContent).toContain('ani-gui made no extra requests');
		expect(noticeCenter.rows[0].read).toBe(true);
	});

	it("carries the notice's action", async () => {
		installWindow({ notification_center: true });
		noticeCenter.raise({
			key: 'update',
			kind: 'update',
			data: { tag: 'v9.0.0', current: '0.20.0' }
		});
		setParams({ key: 'update' });
		app = mount(DetailPage, { target });
		await tick();
		const action = target.querySelector<HTMLButtonElement>('.nb-action');
		expect(action?.textContent).toBe("See what's new");
		action?.click();
		expect(updateStore.dialogOpen).toBe(true);
	});

	it('says a notice is gone when it is, and with the center off', async () => {
		installWindow({ notification_center: true });
		setParams({ key: 'outage:hianime' });
		app = mount(DetailPage, { target });
		await tick();
		expect(target.textContent).toContain('This notification is no longer here.');
		unmount(app);
		installWindow(undefined);
		raiseOutage();
		app = mount(DetailPage, { target });
		await tick();
		expect(target.querySelector('h1')).toBeNull();
		expect(target.textContent).not.toContain('dismissed or resolved');
		expect(target.textContent).toContain('Notifications are turned off.');
	});
});

describe('the line above Play', () => {
	it('says the source is not answering and links to the notice', async () => {
		installWindow({ notification_center: true });
		raiseOutage();
		app = mount(OutageLine, { target });
		await tick();
		expect(target.textContent).toContain("The streaming source hasn't answered since");
		expect(target.textContent).toContain("We don't know why yet");
		expect(target.querySelector('a')?.getAttribute('href')).toBe('/notifications/outage%3Ahianime');
	});

	it('is not there while the source answers, once it answers again, or with the center off', async () => {
		installWindow({ notification_center: true });
		app = mount(OutageLine, { target });
		await tick();
		expect(target.textContent?.trim()).toBe('');
		raiseOutage();
		await tick();
		expect(target.textContent).toContain("hasn't answered");
		noticeCenter.resolve('outage:hianime', Date.now());
		await tick();
		expect(target.textContent?.trim()).toBe('');
		unmount(app);
		resetCenter();
		installWindow(undefined);
		raiseOutage();
		app = mount(OutageLine, { target });
		await tick();
		expect(target.textContent?.trim()).toBe('');
	});
});
