// Changing the external player's type in Settings: the backend says
// what the command becomes, and the page saves the type and that
// command together.

import { describe, expect, it, vi } from 'vitest';
import type { Config, ExternalPlayerKind } from '$lib/api';
import { createKindChange } from './kind-change';

function config(external_player: string, external_player_kind: ExternalPlayerKind): Config {
	return { external_player, external_player_kind } as Config;
}

/** A page whose config is `cfg` and whose save updates it. */
function page(start: Config) {
	let cfg: Config | null = start;
	const saved: Config[] = [];
	return {
		current: () => cfg,
		save: async (next: Config) => {
			cfg = next;
			saved.push(next);
		},
		saved,
		edit: (external_player: string) => {
			cfg = { ...cfg!, external_player };
		}
	};
}

describe('changing the player type', () => {
	it('saves the new type with the command the backend answers', async () => {
		const p = page(config('mpv', 'mpv'));
		const ask = vi.fn(async () => 'vlc');
		await createKindChange(ask, p.current, p.save)('vlc');
		expect(ask).toHaveBeenCalledWith('mpv', 'mpv', 'vlc');
		expect(p.saved).toEqual([config('vlc', 'vlc')]);
	});

	it('keeps the command and still saves the type when the question fails', async () => {
		const p = page(config('mpv', 'mpv'));
		const ask = vi.fn(async () => {
			throw new Error('offline');
		});
		await createKindChange(ask, p.current, p.save)('vlc');
		expect(p.saved).toEqual([config('mpv', 'vlc')]);
	});

	it('asks about each change from where the one before it left the command', async () => {
		const p = page(config('mpv', 'mpv'));
		const answers: Record<string, string> = { 'mpv|mpv|vlc': 'vlc', 'vlc|vlc|iina': 'iina' };
		const ask = vi.fn(async (c: string, from: ExternalPlayerKind, to: ExternalPlayerKind) => {
			await new Promise((r) => setTimeout(r, 5));
			return answers[`${c}|${from}|${to}`] ?? 'unexpected';
		});
		const change = createKindChange(ask, p.current, p.save);
		const first = change('vlc');
		const second = change('iina');
		await Promise.all([first, second]);
		expect(ask.mock.calls).toEqual([
			['mpv', 'mpv', 'vlc'],
			['vlc', 'vlc', 'iina']
		]);
		expect(p.saved.at(-1)).toEqual(config('iina', 'iina'));
	});

	it('keeps a command the user typed while the answer was on its way', async () => {
		const p = page(config('mpv', 'mpv'));
		const ask = vi.fn(async () => {
			p.edit('/opt/mpv/mpv');
			return 'vlc';
		});
		await createKindChange(ask, p.current, p.save)('vlc');
		expect(p.saved).toEqual([config('/opt/mpv/mpv', 'vlc')]);
	});

	it('goes on with the next change after a save that failed', async () => {
		const p = page(config('mpv', 'mpv'));
		let fail = true;
		const save = async (next: Config) => {
			if (fail) {
				fail = false;
				throw new Error('disk full');
			}
			await p.save(next);
		};
		const change = createKindChange(async (_c, _f, to) => to, p.current, save);
		await change('vlc').catch(() => {});
		await change('iina');
		expect(p.saved.at(-1)?.external_player_kind).toBe('iina');
	});

	it('does nothing before the settings have loaded', async () => {
		const ask = vi.fn(async () => 'vlc');
		const save = vi.fn(async () => {});
		await createKindChange(ask, () => null, save)('vlc');
		expect(ask).not.toHaveBeenCalled();
		expect(save).not.toHaveBeenCalled();
	});
});
