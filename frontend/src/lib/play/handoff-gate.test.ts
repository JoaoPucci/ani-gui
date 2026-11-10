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
		gate.handOff();
		gate.comeBack();
		expect(restart).toHaveBeenCalledTimes(1);
	});

	it('keeps only the latest held start, and forgets a dropped one', () => {
		const restart = vi.fn();
		const gate = new HandoffGate(restart);
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
