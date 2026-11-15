/**
 * How a player spec starts playback: as a viewer does, by clicking the
 * home page's Continue card, on a stream the spec serves itself.
 *
 * The click runs the app's own playback path end to end. The renderer
 * asks the backend's resolve lane to play the card's episode
 * (`/api/play/stream`); the backend answers from its resolution cache,
 * checks that the stream there still answers, opens the session and
 * hands it back; the renderer opens the player on it, and the player
 * fetches through the backend's stream proxy. The one step a spec
 * cannot take for real is the provider walk that would have put the row
 * in the cache, since the backend is kept off the network: so the
 * profile turns replays on (`writeReplayConfig`), and the play request
 * is let through to the backend only once the row it reads has been
 * written, naming the spec's stream (lib/e2e-replay.cjs).
 *
 * Everything else the pages ask the API is answered by the stubs
 * below, through stubs.ts's `answer`: the show, its episode, a history with the show on it so
 * the Continue card is there to click, and the settings the spec names.
 * The backend reads its own settings from the profile's config.
 */
import { expect, type Page } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import {
	REPLAY_CONFIG,
	metadataDb,
	replayKey,
	replayRow,
	seedReplay
} from '../lib/e2e-replay.cjs';
import {
	appInfo,
	continueHistory,
	continueKitsuEpisode6,
	continueKitsuMatch,
	defaultSettings,
	topRated,
	trending
} from './fixtures/kitsu';
import { remountFirstPage } from './launch';
import { answer } from './stubs';

/** The episode the Continue card plays: the history's last (5), plus
 *  one. */
export const CONTINUE_EPISODE = 6;

/** The stream a play is answered with. */
export interface ReplayStream {
	upstreamUrl: string;
	mediaKind: 'hls' | 'mp4';
	/** Sidecar tracks the session lists, fetched by the backend from
	 *  these URLs. */
	subtitles?: { lang: string; label: string; default: boolean; url: string }[];
}

/** Writes the profile's config with replays on, after `before`'s
 *  lines. `profile` is the directory the launch's XDG_CONFIG_HOME is
 *  `config` under. */
export function writeReplayConfig(profile: string, before = ''): void {
	const dir = path.join(profile, 'config', 'ani-gui');
	fs.mkdirSync(dir, { recursive: true });
	fs.writeFileSync(path.join(dir, 'config.toml'), `${before}${REPLAY_CONFIG}`);
}

export interface RealPlay {
	/** The cache keys written, one per play of the card's episode. */
	seeded: string[];
}

/**
 * Answers the pages' API requests with stubs, and lets every play
 * request through to the backend — the card's episode's once its row
 * is written. Then mounts the first page again, so the home page asks
 * through the stubs. `profile` is the launch's profile directory
 * (XDG_CACHE_HOME is `cache` under it).
 *
 * A play of any other episode — the warm the player sends for the next
 * one — reaches the backend with no row to read and resolves afresh,
 * which the offline backend cannot do; nothing the specs assert rides
 * on it.
 */
export async function routeRealPlay(
	page: Page,
	{
		profile,
		settings,
		stream
	}: { profile: string; settings: Record<string, unknown>; stream: ReplayStream }
): Promise<RealPlay> {
	const db = metadataDb(path.join(profile, 'cache'));
	const seeded: string[] = [];
	let homeAsked: (() => void) | null = null;
	await page.route('**/api/**', async (r) => {
		const u = new URL(r.request().url());
		const p = u.pathname;
		if (p === '/api/play/stream' || p === '/api/play') {
			const asked =
				p === '/api/play/stream'
					? u.searchParams
					: (JSON.parse(r.request().postData() ?? '{}') as Record<string, unknown>);
			const episode =
				asked instanceof URLSearchParams ? asked.get('episode') : String(asked.episode ?? '');
			if (episode === String(CONTINUE_EPISODE)) {
				const key = replayKey(asked);
				seedReplay(db, key, replayRow({ ...stream, episode: CONTINUE_EPISODE }));
				seeded.push(key);
			}
			return r.continue();
		}
		const j = (body: unknown) =>
			answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
		// A hand-off is never let through: with replays on, the backend
		// would launch the configured player on the seeded row. It answers
		// as the backend does when that player will not start.
		if (p === '/api/play/external' || p === '/api/play/syncplay') {
			const which = p === '/api/play/external' ? 'player' : 'syncplay';
			return answer(r, {
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({
					kind: `${which}_spawn_failed`,
					key: `error.${which}.spawn_failed`,
					binary: which === 'player' ? 'mpv' : 'syncplay'
				})
			});
		}
		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') return j({ ...defaultSettings, ...settings });
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j(topRated);
		if (p.startsWith('/api/image')) return answer(r, { status: 503 });
		if (p === '/api/history') {
			homeAsked?.();
			return j(continueHistory);
		}
		if (p === '/api/watched-at') return j({ [continueHistory[0].id]: 1_700_000_000 });
		if (p.startsWith('/api/allmanga-kitsu-map/')) {
			return j(p.includes(continueHistory[0].id) ? continueKitsuMatch.id : null);
		}
		if (p.startsWith('/api/kitsu/anime/')) return j(continueKitsuMatch);
		if (p.startsWith('/api/kitsu/episodes/')) return j([continueKitsuEpisode6]);
		if (p === '/api/kitsu/search') return j([]);
		if (p === '/api/availability') {
			return j({
				available: true,
				episode_count: continueKitsuMatch.episode_count,
				extra_episodes: []
			});
		}
		// The backend answers skip times with a list, empty when it knows
		// none; the page reads it as one once the length is known.
		if (p.startsWith('/api/aniskip/')) return j([]);
		return j(null);
	});
	await remountFirstPage(page, () => new Promise<void>((resolve) => (homeAsked = resolve)));
	return { seeded };
}

/**
 * Clicks the Continue card and waits for the player's page. The strip
 * is found by its name in English or in Brazilian Portuguese, the two
 * locales the specs run in.
 */
export async function playFromContinue(page: Page, play: RealPlay): Promise<void> {
	const strip = page.getByRole('region', { name: /continue watching|continuar assistindo/i });
	const card = strip.getByRole('button').first();
	await expect(card).toContainText(String(CONTINUE_EPISODE), { timeout: 30_000 });
	await card.click();
	await page.waitForURL(/\/play\//, { timeout: 30_000 });
	expect(play.seeded.length, 'the click asked the backend to play the card').toBeGreaterThan(0);
	// The session is the backend's replay of the seeded row: a session
	// it opened any other way would carry no cache_hit.
	expect(
		new URL(page.url()).searchParams.get('cache_hit'),
		'the backend played the card from the seeded row'
	).toBe('1');
}
