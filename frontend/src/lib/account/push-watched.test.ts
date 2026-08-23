import { describe, expect, it, vi } from 'vitest';
import { pushWatchedToTrackers } from './push-watched';
import type { Provider } from './types';

function deps(overrides: Partial<Parameters<typeof pushWatchedToTrackers>[0]> = {}) {
	const calls: Array<{ provider: Provider; bearer: string; body: unknown }> = [];
	const base = {
		connected: ['anilist', 'mal'] as Provider[],
		bearerFor: (p: Provider) => `bearer-${p}`,
		updateProgress: vi.fn(async (provider: Provider, bearer: string, body: unknown) => {
			calls.push({ provider, bearer, body });
			return null;
		})
	};
	return { d: { ...base, ...overrides }, calls };
}

describe('pushWatchedToTrackers', () => {
	it('fans out to every connected provider with kitsu_id + progress and no status override', async () => {
		// Codex P2 #3387319861: a normal progress update sends no status,
		// so it never downgrades a rewatching/paused tracker row.
		const { d, calls } = deps();
		await pushWatchedToTrackers(d, 'kitsu-12', 7);
		expect(calls).toHaveLength(2);
		expect(calls[0]).toEqual({
			provider: 'anilist',
			bearer: 'bearer-anilist',
			body: { kitsu_id: 'kitsu-12', progress: 7 }
		});
		expect(calls[1].provider).toBe('mal');
	});

	it('skips providers with no bearer (orphaned token)', async () => {
		const { d, calls } = deps({ bearerFor: (p) => (p === 'mal' ? null : 'bearer-anilist') });
		await pushWatchedToTrackers(d, 'kitsu-12', 1);
		expect(calls.map((c) => c.provider)).toEqual(['anilist']);
	});

	it('is best-effort: one provider failing does not block the others or throw', async () => {
		const calls: Provider[] = [];
		const d = {
			connected: ['anilist', 'mal'] as Provider[],
			bearerFor: (p: Provider) => `bearer-${p}`,
			updateProgress: vi.fn(async (provider: Provider) => {
				calls.push(provider);
				if (provider === 'anilist') throw new Error('network');
				return null;
			})
		};
		await expect(pushWatchedToTrackers(d, 'kitsu-12', 3)).resolves.toBeUndefined();
		expect(calls.sort()).toEqual(['anilist', 'mal']);
	});

	it('no-ops on empty kitsu_id or no connected providers', async () => {
		const { d: d1, calls: c1 } = deps();
		await pushWatchedToTrackers(d1, '', 5);
		expect(c1).toHaveLength(0);
		const { d: d2, calls: c2 } = deps({ connected: [] });
		await pushWatchedToTrackers(d2, 'kitsu-12', 5);
		expect(c2).toHaveLength(0);
	});
});

describe('pushWatchedToTrackers finale status', () => {
	it('marks completed on the finale of a finished finite series', async () => {
		// Codex P2 #3386988961: episode N of an N-episode show should
		// move the tracker to completed. Gated on the series being
		// finished, and the only case that sends a status at all.
		const { d, calls } = deps();
		await pushWatchedToTrackers(d, 'kitsu-12', 12, 12, true);
		expect(calls[0].body).toEqual({ kitsu_id: 'kitsu-12', progress: 12, status: 'completed' });
	});

	it('sends no status at the latest episode of a still-airing series', async () => {
		// Codex P2 #3387184082: the playable cap for an airing show is
		// the latest released episode, so episode >= cap is true — but a
		// non-finished series must not be completed (progress only).
		const { d, calls } = deps();
		await pushWatchedToTrackers(d, 'kitsu-12', 12, 12, false);
		expect(calls[0].body).toEqual({ kitsu_id: 'kitsu-12', progress: 12 });
	});

	it('sends no status mid-series', async () => {
		const { d, calls } = deps();
		await pushWatchedToTrackers(d, 'kitsu-12', 6, 12, true);
		expect(calls[0].body).toEqual({ kitsu_id: 'kitsu-12', progress: 6 });
	});

	it('sends no status when episode_count is unknown (null/0)', async () => {
		const { d: d1, calls: c1 } = deps();
		await pushWatchedToTrackers(d1, 'kitsu-12', 6, null, true);
		expect(c1[0].body).toEqual({ kitsu_id: 'kitsu-12', progress: 6 });
		const { d: d2, calls: c2 } = deps();
		await pushWatchedToTrackers(d2, 'kitsu-12', 6, 0, true);
		expect(c2[0].body).toEqual({ kitsu_id: 'kitsu-12', progress: 6 });
	});
});
