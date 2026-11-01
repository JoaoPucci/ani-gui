// Placeholder signatures; the behaviour lands with the green commit.
export const SPOTLIGHT_MS = 3200;

export interface SpotlightHost {
	findTile(episode: number): Element | null;
	scrollTo(tile: Element): void;
	nextFrame(cb: () => void): number;
	cancelFrame(id: number): void;
	setTimer(cb: () => void, ms: number): number;
	clearTimer(id: number): void;
}

export function episodeTileSelector(episode: number): string {
	void episode;
	return '';
}

export function browserSpotlightHost(): SpotlightHost {
	return {
		findTile: () => null,
		scrollTo: () => {},
		nextFrame: () => 0,
		cancelFrame: () => {},
		setTimer: () => 0,
		clearTimer: () => {}
	};
}

export function spotlightEpisode(
	episode: number,
	onEnd: () => void,
	host: SpotlightHost = browserSpotlightHost()
): () => void {
	void [episode, onEnd, host];
	return () => {};
}
