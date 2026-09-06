import { describe, expect, it } from 'vitest';
import { sidecarTracksUrl, trackAttributes, type SidecarTrack } from './sidecar-tracks';

// The play page keeps a session id, not the session response, so the
// tracks are asked for by id — from the same proxy origin the media
// URL is built against.
describe('sidecarTracksUrl', () => {
	it("asks the proxy for the session's track listing", () => {
		expect(sidecarTracksUrl('http://127.0.0.1:31337', 'session-1')).toBe(
			'http://127.0.0.1:31337/s/session-1/subtitles'
		);
	});

	it('tolerates a trailing slash on the origin and escapes the id', () => {
		expect(sidecarTracksUrl('http://127.0.0.1:31337/', 'a b')).toBe(
			'http://127.0.0.1:31337/s/a%20b/subtitles'
		);
	});
});

// Each listed track becomes one <track> — a subtitle track the browser
// renders natively and the captions picker already lists.
describe('trackAttributes', () => {
	const track: SidecarTrack = {
		lang: 'en',
		label: 'English',
		default: true,
		url: 'http://127.0.0.1:31337/s/session-1/sub/0.vtt'
	};

	it("maps a listed track onto the element's attributes", () => {
		expect(trackAttributes(track)).toEqual({
			kind: 'subtitles',
			srclang: 'en',
			label: 'English',
			src: 'http://127.0.0.1:31337/s/session-1/sub/0.vtt',
			default: true
		});
	});

	it('marks only the track the provider defaulted', () => {
		expect(trackAttributes({ ...track, default: false }).default).toBe(false);
	});
});
