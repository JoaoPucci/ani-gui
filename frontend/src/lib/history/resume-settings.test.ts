import { describe, expect, it } from 'vitest';
import { resolveResumeSettings } from './resume-settings';
import type { Config } from '$lib/api';

function cfg(mode: string): Config {
	return { mode } as unknown as Config;
}

describe('resolveResumeSettings', () => {
	it('uses the already-loaded config without touching the promise', async () => {
		// The rejection is pre-handled: the short-circuit means nothing
		// ever awaits this promise, and an unhandled rejection would
		// fail the run at the harness level.
		const untouched = Promise.reject(new Error('must not matter'));
		untouched.catch(() => {});
		const r = await resolveResumeSettings(cfg('dub'), untouched);
		expect(r).toEqual({ mode: 'dub' });
	});

	it('awaits the shared settings promise when config has not landed', async () => {
		// A cached Kitsu match can release its
		// card before settingsGet resolves. Capturing the sub
		// fallback at that instant gives a DUB user the wrong variant
		// for both the cap lookup and playback; the click must wait
		// for the same shared settings the loader awaits.
		const r = await resolveResumeSettings(null, Promise.resolve(cfg('dub')));
		expect(r).toEqual({ mode: 'dub' });
	});

	it('falls back to sub when settings never load', async () => {
		const r = await resolveResumeSettings(null, Promise.reject(new Error('settings down')));
		expect(r).toEqual({ mode: 'sub' });
	});

	it('falls back to sub on a null settings resolution', async () => {
		const r = await resolveResumeSettings(null, Promise.resolve(null));
		expect(r).toEqual({ mode: 'sub' });
	});
});
