import { describe, expect, it } from 'vitest';
import { describeExternalLaunchFailure } from './external-launch-copy';

describe('describeExternalLaunchFailure', () => {
	it('names the configured binary when the spawn failed', () => {
		// The user's specific complaint: the small inline notice was
		// easy to miss. The modal-driven copy still has to surface
		// *which* command went missing so the user can fix it in
		// settings without guessing. The backend ships the binary in
		// the typed payload.
		const msg = describeExternalLaunchFailure({
			kind: 'player_spawn_failed',
			binary: 'mpv'
		});
		expect(msg).toMatch(/mpv/);
	});

	it('includes a hint about PATH / settings for the spawn-failed case', () => {
		// The body needs to point the user at *what to do next* —
		// install the player or pick its full path in settings —
		// otherwise the modal just relabels the failure without
		// helping. Pin a stable substring so a copy refresh that
		// drops the actionable hint gets caught.
		const msg = describeExternalLaunchFailure({
			kind: 'player_spawn_failed',
			binary: 'vlc'
		});
		expect(msg).toMatch(/PATH|Settings/i);
	});

	it('falls back to describePlayFailure copy for non-spawn errors', () => {
		// External launch resolves the same upstream URL as embedded
		// play, so it can hit the same scraper / network branches.
		// Reusing describePlayFailure keeps the user-facing copy
		// consistent across both surfaces (no debug-y "External
		// player failed: scraper" leak).
		expect(describeExternalLaunchFailure({ kind: 'timeout' })).toMatch(/took too long/);
		expect(describeExternalLaunchFailure({ kind: 'network' })).toMatch(/Network trouble/);
	});

	it('rejects payloads with the wrong shape and falls back to generic copy', () => {
		// Defensive: a backend drift that drops `binary` or sends a
		// non-string shouldn't crash the renderer. The generic
		// "try again" message is the right safety net.
		expect(describeExternalLaunchFailure({ kind: 'player_spawn_failed' })).toMatch(
			/Couldn't start this episode right now/
		);
		expect(describeExternalLaunchFailure({ kind: 'player_spawn_failed', binary: '' })).toMatch(
			/Couldn't start this episode right now/
		);
	});
});
