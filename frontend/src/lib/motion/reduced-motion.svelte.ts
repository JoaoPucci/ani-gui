// Placeholder for the reduced-motion source: the signatures its tests
// need to type-check, and nothing behind them yet.
/* eslint-disable @typescript-eslint/no-unused-vars -- placeholder signatures the next commit fills in */
export const REDUCED_MOTION_QUERY = '';

export interface MotionQuery {
	readonly matches: boolean;
	addEventListener(type: 'change', listener: () => void): void;
	removeEventListener(type: 'change', listener: () => void): void;
}

export interface ReducedMotion {
	readonly current: boolean;
	onChange(listener: (reduced: boolean) => void): () => void;
}

export function systemMotionQuery(
	_win: { matchMedia?: (query: string) => MotionQuery } | undefined
): MotionQuery | null {
	throw new Error('not implemented');
}

export function createReducedMotion(_query: MotionQuery | null): ReducedMotion {
	throw new Error('not implemented');
}

export const reducedMotion: ReducedMotion = {
	get current(): boolean {
		throw new Error('not implemented');
	},
	onChange() {
		throw new Error('not implemented');
	}
};
