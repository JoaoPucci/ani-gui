import { describe, expect, it, vi } from 'vitest';
import { createMiniSessions, type MiniSession } from './session';

function session(restore: boolean) {
	const video = { pause: vi.fn() } as unknown as HTMLVideoElement;
	const s: MiniSession = {
		video,
		show: { id: '46474', title: 'Frieren', episode: 12, accent: '#c33', playHref: '/play/46474' },
		hooks: { dispose: vi.fn(), restore: restore ? vi.fn() : null }
	};
	return s;
}

describe('the mockup mini-player session', () => {
	it('holds the video it was handed until it ends', () => {
		const sessions = createMiniSessions();
		const s = session(true);
		expect(sessions.current).toBeNull();
		sessions.open(s);
		expect(sessions.current).toBe(s);
		expect(sessions.holds(s.video)).toBe(true);
		expect(sessions.holds({} as HTMLVideoElement)).toBe(false);
	});

	it('gives the video back to a play page that is still there', () => {
		const sessions = createMiniSessions();
		const s = session(true);
		sessions.open(s);
		expect(sessions.returnToApp()).toBe('restored');
		expect(s.hooks.restore).toHaveBeenCalledOnce();
		expect(s.hooks.dispose).not.toHaveBeenCalled();
		expect(sessions.current).toBeNull();
	});

	it('ends playback and asks for the play page once that page is gone', () => {
		const sessions = createMiniSessions();
		const s = session(true);
		sessions.open(s);
		sessions.pageGone(s.video);
		expect(sessions.returnToApp()).toBe('navigate');
		expect(s.hooks.dispose).toHaveBeenCalledOnce();
		expect(s.hooks.restore).not.toHaveBeenCalled();
	});

	it('closing stops playback: paused in a page still there, ended otherwise', () => {
		const sessions = createMiniSessions();
		const kept = session(true);
		sessions.open(kept);
		sessions.close();
		expect(kept.video.pause).toHaveBeenCalledOnce();
		expect(kept.hooks.restore).toHaveBeenCalledOnce();
		expect(sessions.current).toBeNull();

		const gone = session(false);
		sessions.open(gone);
		sessions.close();
		expect(gone.hooks.dispose).toHaveBeenCalledOnce();
	});

	it('closes the session before it when another opens: one engine', () => {
		const sessions = createMiniSessions();
		const first = session(false);
		const second = session(false);
		sessions.open(first);
		sessions.open(second);
		expect(first.hooks.dispose).toHaveBeenCalledOnce();
		expect(sessions.current).toBe(second);
	});

	it('does nothing when there is nothing to end, or the page is another', () => {
		const sessions = createMiniSessions();
		expect(sessions.returnToApp()).toBeNull();
		expect(() => sessions.close()).not.toThrow();
		const s = session(true);
		sessions.open(s);
		sessions.pageGone({} as HTMLVideoElement);
		expect(sessions.returnToApp()).toBe('restored');
	});

	it('tells its listeners each change, until they leave', () => {
		const sessions = createMiniSessions();
		const seen: (MiniSession | null)[] = [];
		const leave = sessions.subscribe((s) => seen.push(s));
		const s = session(true);
		sessions.open(s);
		sessions.close();
		leave();
		sessions.open(session(true));
		expect(seen).toEqual([null, s, null]);
	});
});
