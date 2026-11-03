// A published notice as a row: how it is raised, drawn, counted,
// announced, dismissed and acted on.
import { describe, expect, it, vi } from 'vitest';
import { arrival, IDLE } from './arrival';
import { copyOf, toastOf } from './copy';
import { canDismiss, isKind, lifetimeOf, raise, toneOf, unreadCount, type Notice } from './model';
import { act } from './panel';
import { serialize } from './storage';

const NOW = 1_800_000_000_000;

function published(over: Record<string, string | null> = {}, read = false): Notice {
	const c = raise(
		[],
		{
			key: 'remote:maint',
			kind: 'remote',
			read,
			data: {
				title: 'Maintenance tonight',
				body: 'Back by 22:00.',
				link: 'https://example.org/m',
				priority: 'warning',
				...over
			}
		},
		NOW
	);
	return c.notice as Notice;
}

describe('a published notice', () => {
	it('is a kind of its own that lives while the file publishes it and is never stored', () => {
		expect(isKind('remote')).toBe(true);
		expect(lifetimeOf('remote')).toBe('condition');
		expect(JSON.parse(serialize([published()])).rows).toEqual([]);
	});

	it('arrives as news, unless an earlier launch already read it', () => {
		const fresh = raise([], { key: 'remote:a', kind: 'remote', data: { title: 'A' } }, NOW);
		expect(fresh.fresh).toBe(true);
		expect(fresh.notice?.read).toBe(false);
		const seen = raise(
			[],
			{ key: 'remote:a', kind: 'remote', read: true, data: { title: 'A' } },
			NOW
		);
		expect(seen.fresh).toBe(false);
		expect(seen.notice?.read).toBe(true);
		expect(unreadCount([seen.notice as Notice], { updateOnBadge: false })).toBe(0);
	});

	it('takes its tone from its priority', () => {
		expect(toneOf(published({ priority: 'info' }))).toBe('info');
		expect(toneOf(published({ priority: 'warning' }))).toBe('live');
		expect(toneOf(published({ priority: 'critical' }))).toBe('failure');
		expect(toneOf(published({ priority: 'loud' }))).toBe('info');
	});

	it('may be dismissed', () => {
		expect(canDismiss(published())).toBe(true);
	});

	it('toasts when it arrives, but waits while a video plays, whatever its priority', () => {
		const critical = published({ priority: 'critical' });
		expect(arrival(critical, true, IDLE)).toBe('toast');
		expect(arrival(critical, true, { playing: true, fullscreen: false })).toBe('quiet');
		expect(arrival(critical, true, { playing: false, fullscreen: true })).toBe('hold');
	});

	it('says what the file says, as text, with its link shown and opened by its action', () => {
		const c = copyOf(published());
		expect(c.title).toBe('Maintenance tonight');
		expect(c.meta).toBe('From the developer');
		expect(c.body).toEqual([
			{ label: null, text: 'Back by 22:00.' },
			{ label: 'Link', text: 'https://example.org/m' }
		]);
		expect(c.action).toEqual({ kind: 'open-link', label: 'Open link' });
		expect(toastOf(published()).message).toBe('Maintenance tonight');
	});

	it('without a body or link says only its title, and has no action', () => {
		const c = copyOf(published({ body: null, link: null }));
		expect(c.body).toEqual([]);
		expect(c.action).toBeNull();
	});

	it("opens its link in the user's browser", () => {
		const openLink = vi.fn();
		const deps = { openUpdate: vi.fn(), reveal: vi.fn(), openLink };
		act('open-link', published(), deps);
		expect(openLink).toHaveBeenCalledWith('https://example.org/m');
		act('open-link', published({ link: null }), deps);
		expect(openLink).toHaveBeenCalledTimes(1);
	});
});

describe('the outage row with a notice about the source', () => {
	const outage = (data: Record<string, string | number | null>) =>
		raise(
			[],
			{
				key: 'outage:hianime',
				kind: 'outage',
				data: { since: NOW - 3600_000, failures: 4, last: NOW - 60_000, ...data }
			},
			NOW
		).notice as Notice;

	it("carries the developer's words after what the app saw, and its link", () => {
		const c = copyOf(
			outage({
				noteTitle: 'Down for maintenance',
				noteBody: 'Back tonight.',
				noteLink: 'https://example.org/s'
			}),
			NOW
		);
		expect(c.body.at(-2)).toEqual({
			label: 'From the developer',
			text: 'Down for maintenance\nBack tonight.'
		});
		expect(c.body.at(-1)).toEqual({ label: 'Link', text: 'https://example.org/s' });
		expect(c.action).toEqual({ kind: 'open-link', label: 'Open link' });
		const openLink = vi.fn();
		act('open-link', outage({ noteTitle: 'x', noteLink: 'https://example.org/s' }), {
			openUpdate: vi.fn(),
			reveal: vi.fn(),
			openLink
		});
		expect(openLink).toHaveBeenCalledWith('https://example.org/s');
	});

	it('without one is as before', () => {
		const c = copyOf(outage({}), NOW);
		expect(c.body).toHaveLength(3);
		expect(c.action).toBeNull();
	});
});
