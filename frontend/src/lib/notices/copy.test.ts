import { describe, expect, it } from 'vitest';
import { copyOf, detailPath, toastOf, when } from './copy';
import { raise, resolveKey, type Notice, type RaiseInput } from './model';

// A fixed local noon, so "today" and "this week" do not depend on when
// the test runs.
const NOW = new Date(2026, 10, 3, 12, 0, 0).getTime();
const HOUR = 3600_000;

function make(input: RaiseInput, at = NOW): Notice {
	return raise([], input, at).notice as Notice;
}

describe('times on a row', () => {
	it('shows the clock time today, the weekday this week, the date before', () => {
		expect(when(NOW - HOUR, NOW, 'en')).toMatch(/^11:00\s?AM$/);
		expect(when(NOW - 2 * 24 * HOUR, NOW, 'en')).toMatch(/^Sun,? 12:00\s?PM$/);
		expect(when(NOW - 20 * 24 * HOUR, NOW, 'en')).toBe('Oct 14');
	});

	it('follows the locale', () => {
		expect(when(NOW - HOUR, NOW, 'ru')).toBe('11:00');
	});

	it('defaults to now and the current locale', () => {
		expect(when(Date.now())).toMatch(/\d/);
	});
});

describe('what each notice says', () => {
	it('the update names both versions and opens the update', () => {
		const c = copyOf(
			make({ key: 'update', kind: 'update', data: { tag: 'v0.20.1', current: '0.20.0' } })
		);
		expect(c.title).toBe('ani-gui v0.20.1 is available');
		expect(c.meta).toBe('You have 0.20.0');
		expect(c.action).toEqual({ kind: 'open-update', label: "See what's new" });
		expect(c.body).toHaveLength(1);
	});

	it('the outage says plainly that the cause is not known, and what was seen', () => {
		const n = make({
			key: 'outage:hianime',
			kind: 'outage',
			data: { since: NOW - HOUR, failures: 6, last: NOW - 5 * 60_000 }
		});
		const c = copyOf(n, NOW);
		expect(c.title).toBe("The streaming source isn't answering");
		expect(c.meta).toMatch(/^Since 11:00\s?AM · 6 attempts failed$/);
		expect(c.body[0].text).toContain("We don't know why yet");
		expect(c.body[2].label).toBe('What ani-gui saw');
		expect(c.body[2].text).toMatch(/6 attempts in a row .* the latest: 11:55\s?AM\./);
		expect(c.body[2].text).toContain('no extra requests');
		expect(c.action).toBeNull();
	});

	it('an outage without its times falls back to the row', () => {
		const c = copyOf(make({ key: 'outage:x', kind: 'outage', count: 0 }, NOW - HOUR), NOW);
		expect(c.meta).toMatch(/^Since 11:00\s?AM · 0 attempts failed$/);
	});

	it('a resolved outage says the source is answering again, and since when', () => {
		const live = make({ key: 'outage:x', kind: 'outage' });
		const n = resolveKey([live], 'outage:x', NOW - HOUR).notice as Notice;
		const c = copyOf(n, NOW);
		expect(c.title).toBe('The streaming source is answering again');
		expect(c.meta).toMatch(/^Back since 11:00\s?AM$/);
		expect(c.body[0].text).toMatch(/answered again \(11:00\s?AM\)/);
	});

	it('finished downloads count their episodes and reveal their folder', () => {
		const c = copyOf(
			make({
				key: 'download-done:F',
				kind: 'download-done',
				count: 3,
				data: { title: 'Frieren', destDir: '/v' }
			})
		);
		expect(c.title).toBe('3 episodes downloaded');
		expect(c.meta).toBe('Frieren');
		expect(c.body).toEqual([{ label: null, text: 'Saved to /v' }]);
		expect(c.action).toEqual({ kind: 'reveal-folder', label: 'Show in folder' });
		const noDir = copyOf(make({ key: 'd', kind: 'download-done', data: { title: 'X' } }));
		expect(noDir.title).toBe('1 episode downloaded');
		expect(noDir.action).toBeNull();
		expect(noDir.body).toEqual([]);
	});

	it('a failed download names the episode and gives its reason, or says it has none', () => {
		const c = copyOf(
			make({
				key: 'f',
				kind: 'download-failed',
				data: { title: 'Frieren', episode: '5', reason: 'The source stopped answering.' }
			})
		);
		expect(c.title).toBe('A download failed');
		expect(c.meta).toBe('Frieren · Episode 5');
		expect(c.body).toEqual([{ label: 'What went wrong', text: 'The source stopped answering.' }]);
		const bare = copyOf(make({ key: 'f', kind: 'download-failed', data: { reason: null } }));
		expect(bare.body[0].text).toBe('The download stopped without saying why.');
	});
});

describe('toasts', () => {
	it('colours a live condition amber and a failed event oxblood, and names a failure', () => {
		const outage = make({ key: 'outage:x', kind: 'outage', data: { since: NOW } });
		expect(toastOf(outage)).toEqual({
			kind: 'warning',
			message: "The streaming source isn't answering"
		});
		const failed = make({
			key: 'f',
			kind: 'download-failed',
			data: { title: 'Frieren', episode: '5' }
		});
		expect(toastOf(failed)).toEqual({
			kind: 'error',
			message: 'A download failed · Frieren · Episode 5'
		});
		const done = make({ key: 'd', kind: 'download-done', data: { title: 'X' } });
		expect(toastOf(done).kind).toBe('info');
		const back = resolveKey([outage], 'outage:x', NOW).notice as Notice;
		expect(toastOf(back).kind).toBe('success');
	});
});

describe('detail pages', () => {
	it('live at an address that carries the key', () => {
		expect(detailPath('outage:hianime')).toBe('/notifications/outage%3Ahianime');
		expect(detailPath('download-done:A/B')).toBe('/notifications/download-done%3AA%2FB');
	});
});
