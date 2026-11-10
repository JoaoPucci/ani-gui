import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { HandoffGate } from './handoff-gate';

describe('HandoffGate', () => {
	it('runs a start at once while the episode plays here', () => {
		const gate = new HandoffGate(vi.fn());
		const start = vi.fn();
		gate.whenWatching(start);
		expect(start).toHaveBeenCalledTimes(1);
	});

	it('holds a start while the episode is handed off, and runs it when the viewer comes back', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
		gate.handOff();
		const start = vi.fn();
		gate.whenWatching(start);
		expect(start).not.toHaveBeenCalled();
		gate.comeBack();
		expect(start).toHaveBeenCalledTimes(1);
		// The held start is the resume: nothing else restarts.
		expect(restart).not.toHaveBeenCalled();
		expect(gate.handedOff).toBe(false);
	});

	it('restarts what the handoff stopped when nothing was held', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
		gate.engineStarted();
		gate.handOff();
		gate.comeBack();
		expect(restart).toHaveBeenCalledTimes(1);
	});

	it('keeps only the latest held start, and forgets a dropped one', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
		gate.engineStarted();
		gate.handOff();
		const first = vi.fn();
		const second = vi.fn();
		gate.whenWatching(first);
		gate.whenWatching(second);
		gate.drop(second);
		gate.comeBack();
		expect(first).not.toHaveBeenCalled();
		expect(second).not.toHaveBeenCalled();
		expect(restart).toHaveBeenCalledTimes(1);
	});

	it('coming back when nothing was handed off does nothing', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
		gate.comeBack();
		expect(restart).not.toHaveBeenCalled();
	});

	it('runs every start exactly once, now or on the way back', () => {
		fc.assert(
			fc.property(
				fc.array(fc.constantFrom('handOff', 'comeBack', 'start'), { maxLength: 30 }),
				(ops) => {
					const gate = new HandoffGate(() => {});
					let ran = 0;
					let made = 0;
					let held = false;
					for (const op of ops) {
						if (op === 'handOff') gate.handOff();
						else if (op === 'comeBack') {
							gate.comeBack();
							held = false;
						} else {
							made += 1;
							const wasHanded = gate.handedOff;
							if (wasHanded && held) made -= 1; // the earlier held start is replaced
							held = wasHanded;
							gate.whenWatching(() => {
								ran += 1;
							});
						}
					}
					gate.comeBack();
					expect(ran).toBe(made);
				}
			)
		);
	});
});

describe('HandoffGate — the viewer picks another stream', () => {
	it('forgets the handoff and the held start without running anything', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
		gate.handOff();
		const start = vi.fn();
		gate.whenWatching(start);
		gate.forget();
		expect(gate.handedOff).toBe(false);
		expect(start).not.toHaveBeenCalled();
		expect(restart).not.toHaveBeenCalled();
		// And nothing is left to run later.
		gate.engineStarted();
		gate.handOff();
		gate.comeBack();
		expect(start).not.toHaveBeenCalled();
		expect(restart).toHaveBeenCalledTimes(1);
	});
});

describe('HandoffGate — the engine it restarts', () => {
	it('restarts only an engine the page started, and not one stopped for good since', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
		gate.handOff();
		gate.comeBack();
		expect(restart).not.toHaveBeenCalled();

		gate.engineStarted();
		gate.engineStopped();
		gate.handOff();
		gate.comeBack();
		expect(restart).not.toHaveBeenCalled();

		gate.engineStarted();
		gate.handOff();
		gate.comeBack();
		expect(restart).toHaveBeenCalledTimes(1);
	});

	it('a stream attaching is a new engine, not started yet', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
		gate.engineStarted();
		gate.attaching();
		gate.handOff();
		gate.comeBack();
		expect(restart).not.toHaveBeenCalled();
	});
});

describe('HandoffGate — a pick from the embedded player', () => {
	it('ends the handoff as the picked stream attaches, and not before', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
		gate.engineStarted();
		gate.handOff();
		gate.picked();
		expect(gate.handedOff).toBe(true);
		expect(gate.attaching()).toBe(true);
		expect(gate.handedOff).toBe(false);
		expect(restart).not.toHaveBeenCalled();
		// Once only.
		expect(gate.attaching()).toBe(false);
	});

	it('a pick made while nothing is handed off ends nothing', () => {
		const gate = new HandoffGate(() => {});
		gate.picked();
		expect(gate.attaching()).toBe(false);
	});

	it('a later handoff, or the viewer playing again, cancels a pick that never attached', () => {
		const gate = new HandoffGate(() => {});
		gate.handOff();
		gate.picked();
		gate.handOff();
		expect(gate.attaching()).toBe(false);
		expect(gate.handedOff).toBe(true);

		gate.picked();
		gate.comeBack();
		gate.handOff();
		expect(gate.attaching()).toBe(false);
		expect(gate.handedOff).toBe(true);
	});
});

describe('HandoffGate — a recovery whose eviction answered', () => {
	it('stands down while handed off, unless it is a Reload the viewer still means', () => {
		const gate = new HandoffGate(() => {});
		expect(gate.recoveryStandsDown(false)).toBe(false);
		expect(gate.recoveryStandsDown(true)).toBe(false);
		gate.handOff();
		expect(gate.recoveryStandsDown(false)).toBe(true);
		expect(gate.recoveryStandsDown(true)).toBe(true);
		gate.picked();
		expect(gate.recoveryStandsDown(true)).toBe(false);
		expect(gate.recoveryStandsDown(false)).toBe(true);
		// A handoff after the Reload was pressed: it no longer goes on.
		gate.handOff();
		expect(gate.recoveryStandsDown(true)).toBe(true);
	});
});

describe('HandoffGate — every sequence', () => {
	type Op = 'handOff' | 'comeBack' | 'picked' | 'attaching' | 'started' | 'stopped' | 'forget';
	it('matches a model of the transitions: restarts, ends and stand-downs', () => {
		fc.assert(
			fc.property(
				fc.array(
					fc.constantFrom<Op>(
						'handOff',
						'comeBack',
						'picked',
						'attaching',
						'started',
						'stopped',
						'forget'
					),
					{ maxLength: 40 }
				),
				(ops) => {
					let restarts = 0;
					const gate = new HandoffGate(() => {
						restarts += 1;
					});
					const model = { off: false, pick: false, engine: false, restarts: 0 };
					for (const op of ops) {
						switch (op) {
							case 'handOff':
								gate.handOff();
								model.off = true;
								model.pick = false;
								break;
							case 'comeBack':
								gate.comeBack();
								if (model.off && model.engine) model.restarts += 1;
								model.off = false;
								model.pick = false;
								break;
							case 'picked':
								gate.picked();
								if (model.off) model.pick = true;
								break;
							case 'attaching': {
								const ended = gate.attaching();
								expect(ended).toBe(model.pick);
								if (model.pick) model.off = false;
								model.pick = false;
								model.engine = false;
								break;
							}
							case 'started':
								gate.engineStarted();
								model.engine = true;
								break;
							case 'stopped':
								gate.engineStopped();
								model.engine = false;
								break;
							case 'forget':
								gate.forget();
								model.off = false;
								model.pick = false;
								break;
						}
						expect(gate.handedOff).toBe(model.off);
						expect(restarts).toBe(model.restarts);
						expect(gate.recoveryStandsDown(false)).toBe(model.off);
						expect(gate.recoveryStandsDown(true)).toBe(model.off && !model.pick);
					}
				}
			)
		);
	});
});
