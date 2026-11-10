/**
 * Whether the episode on the play page has been handed to another
 * player. The signatures only, so far: the behaviour comes with its
 * tests.
 */
export class HandoffGate {
	constructor(restart: () => void) {
		void restart;
	}
	get handedOff(): boolean {
		return false;
	}
	handOff(): void {}
	comeBack(): void {}
	whenWatching(start: () => void): void {
		void start;
	}
	drop(start: () => void): void {
		void start;
	}
}
