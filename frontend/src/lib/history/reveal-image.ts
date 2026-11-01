// Shows a card's artwork. Stub with today's behaviour: the image is
// shown at once, painting in as it streams.

export interface RevealTarget {
	readonly complete: boolean;
	readonly naturalWidth: number;
	readonly dataset: DOMStringMap;
	decode?: () => Promise<void>;
	addEventListener(type: 'load' | 'error', fn: () => void): void;
	removeEventListener(type: 'load' | 'error', fn: () => void): void;
}

export interface RevealOptions {
	onError?: () => void;
}

export function revealWhenDecoded(img: RevealTarget, options: RevealOptions = {}) {
	void options;
	img.dataset.revealed = 'instant';
	return { destroy() {} };
}
