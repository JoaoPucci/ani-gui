// Switching the player to the show picked for its card: the same
// episode, from the same point, on the picked show's own stream.

import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import type { CreateSessionResponse, KitsuAnimeRef, PlayArgs } from '$lib/api';
import { switchShow, type SwitchShowDeps, type SwitchRequest } from './switch-show';

function ref(id: string, title: string | null): KitsuAnimeRef {
	return {
		id,
		canonical_title: title as string,
		slug: null,
		synopsis: null,
		start_date: '2000-03-03',
		end_date: null,
		episode_count: 13,
		average_rating: null,
		subtype: 'OVA',
		status: 'finished',
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null
	};
}

const RIGHT = ref('4013', 'JoJo no Kimyou na Bouken (2000)');
const CARD = 'hianime:jojos-bizarre-adventure-2000-5182';
const SESSION: CreateSessionResponse = {
	session_id: 'session-2',
	media_url: '/s/t/file.mp4',
	media_kind: 'mp4'
};

function deps(over: Partial<SwitchShowDeps> = {}) {
	const calls: string[] = [];
	const d: SwitchShowDeps = {
		resolve: vi.fn(async () => {
			calls.push('resolve');
			return SESSION;
		}),
		record: vi.fn(() => void calls.push('record')),
		carry: vi.fn(() => void calls.push('carry')),
		noteSwitch: vi.fn(() => void calls.push('note')),
		open: vi.fn(async () => {
			calls.push('open');
			return true;
		}),
		played: vi.fn(
			(_showId: string, _session: CreateSessionResponse, requested: number) => requested
		),
		carryRate: vi.fn(),
		forget: vi.fn(() => void calls.push('forget')),
		isCurrent: () => true,
		...over
	};
	return { d, calls };
}

const request = (over: Partial<SwitchRequest> = {}): SwitchRequest => ({
	to: RIGHT,
	episode: 2,
	at: 14.5,
	row: CARD,
	mode: 'sub',
	...over
});

describe('switching to the picked show', () => {
	it("asks for the picked show's own stream of the same episode, by its own title and id", async () => {
		const { d } = deps();
		expect(await switchShow(d, request())).toBe('switched');
		const args = vi.mocked(d.resolve).mock.calls[0][0] as PlayArgs;
		expect(args).toMatchObject({
			title: RIGHT.canonical_title,
			episode: '2',
			mode: 'sub',
			kitsu_id: RIGHT.id,
			subtype: 'OVA',
			year: 2000
		});
		// No guess travels with it: the show is the user's pick.
		expect(args.guess_row).toBeUndefined();
		expect(args.guess_id).toBeUndefined();
	});

	it("asks the backend to write no history on resolving, at a click's priority", async () => {
		// A viewer who leaves before the player opens, or a player that
		// cannot open, leaves the history as it was: the watch is recorded
		// once the player opened. Not a warm, so not a prefetch.
		const { d } = deps();
		await switchShow(d, request());
		const args = vi.mocked(d.resolve).mock.calls[0][0] as PlayArgs;
		expect(args.record_later).toBe(true);
		expect(args.prefetch).toBeUndefined();
		const recorded = vi.mocked(d.record).mock.calls[0][0] as PlayArgs;
		expect(recorded.record_later).toBeUndefined();
	});

	it('records the play, carries the point over, tells the next session, then opens it', async () => {
		const { d, calls } = deps();
		await switchShow(d, request());
		// The next page is told before it opens; the play is recorded once
		// it has.
		expect(calls).toEqual(['resolve', 'carry', 'note', 'open', 'record']);
		expect(d.record).toHaveBeenCalledWith(
			expect.objectContaining({ kitsu_id: RIGHT.id }),
			SESSION,
			2
		);
		expect(d.carry).toHaveBeenCalledWith(RIGHT.id, 2, 14.5);
		expect(d.noteSwitch).toHaveBeenCalledWith(CARD, RIGHT.canonical_title);
		expect(d.open).toHaveBeenCalledWith(RIGHT.id, SESSION, 2, CARD);
	});

	it('changes nothing when the stream cannot be had', async () => {
		const { d, calls } = deps({
			resolve: vi.fn(async () => Promise.reject(new Error('no stream')))
		});
		expect(await switchShow(d, request())).toBe('failed');
		expect(calls).toEqual([]);
	});

	it('changes nothing for a viewer who left the player meanwhile', async () => {
		const { d, calls } = deps({ isCurrent: () => false });
		expect(await switchShow(d, request())).toBe('abandoned');
		expect(calls).toEqual(['resolve']);
	});

	it('fails at once for a show with no title to ask by', async () => {
		const { d, calls } = deps();
		expect(await switchShow(d, request({ to: ref('4013', null) }))).toBe('failed');
		expect(calls).toEqual([]);
	});

	it('tells no session about a switch made from no card', async () => {
		const { d } = deps();
		await switchShow(d, request({ row: null }));
		expect(d.noteSwitch).not.toHaveBeenCalled();
		expect(d.open).toHaveBeenCalledWith(RIGHT.id, SESSION, 2, null);
	});

	it('follows the episode the session says it plays, for the record, the point and the player', async () => {
		// The resolve may name the episode by the number it has now: a
		// recap renumbered from 24.5 to 12.5.
		const { d } = deps({ played: vi.fn(() => 12.5) });
		await switchShow(d, request({ episode: 24.5 }));
		expect(d.played).toHaveBeenCalledWith(RIGHT.id, SESSION, 24.5);
		expect((vi.mocked(d.record).mock.calls[0][0] as PlayArgs).episode).toBe('12.5');
		// The trackers hear the episode played too.
		expect(vi.mocked(d.record).mock.calls[0][2]).toBe(12.5);
		expect(d.carry).toHaveBeenCalledWith(RIGHT.id, 12.5, 14.5);
		expect(d.open).toHaveBeenCalledWith(RIGHT.id, SESSION, 12.5, CARD);
	});

	it('keeps the playback speed for the next player', async () => {
		const { d } = deps();
		await switchShow(d, request({ rate: 1.5 }));
		expect(d.carryRate).toHaveBeenCalledWith(RIGHT.id, 1.5);
	});

	it('fails when the player cannot be opened on the stream, and leaves nothing behind', async () => {
		const { d, calls } = deps({
			open: vi.fn(async () => {
				calls.push('open');
				return false;
			})
		});
		expect(await switchShow(d, request())).toBe('failed');
		// No watch of a show that never played, and nothing left for a
		// later page to take.
		expect(d.record).not.toHaveBeenCalled();
		expect(calls.at(-1)).toBe('forget');
		expect(d.forget).toHaveBeenCalledWith(RIGHT.id, CARD);
	});

	it('fails, leaving nothing behind, when opening the player throws before it navigates', async () => {
		const { d } = deps({
			open: vi.fn(() => {
				throw new Error('no route');
			})
		});
		expect(await switchShow(d, request())).toBe('failed');
		expect(d.record).not.toHaveBeenCalled();
		expect(d.forget).toHaveBeenCalledWith(RIGHT.id, CARD);
	});

	it('has switched once the player opened, whatever recording the watch does', async () => {
		const { d } = deps({
			record: vi.fn(() => {
				throw new Error('record refused');
			})
		});
		expect(await switchShow(d, request())).toBe('switched');
		expect(d.forget).not.toHaveBeenCalled();
	});

	it('carries over exactly the episode and point it was asked for', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.integer({ min: 1, max: 2000 }),
				fc.double({ min: 0, max: 10_000, noNaN: true }),
				async (episode, at) => {
					const { d } = deps();
					await switchShow(d, request({ episode, at }));
					expect(d.carry).toHaveBeenCalledWith(RIGHT.id, episode, at);
					expect((vi.mocked(d.resolve).mock.calls[0][0] as PlayArgs).episode).toBe(String(episode));
				}
			)
		);
	});
});
