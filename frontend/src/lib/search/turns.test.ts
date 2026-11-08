import { describe, expect, it, vi } from 'vitest';
import type { ProbeRun } from '$lib/availability/probe-scope';
import { searchTurns } from './turns';

function probes(): ProbeRun {
	return { end: vi.fn(), signal: new AbortController().signal } as unknown as ProbeRun;
}

describe('searchTurns', () => {
	it('makes each new turn the only current one, ending the last one`s probes', () => {
		const made: ProbeRun[] = [];
		const turns = searchTurns(() => {
			const p = probes();
			made.push(p);
			return p;
		});
		const a = turns.take();
		expect(a.current()).toBe(true);
		const b = turns.take();
		expect(a.current()).toBe(false);
		expect(b.current()).toBe(true);
		expect(made[0].end).toHaveBeenCalledTimes(1);
		expect(b.probes).toBe(made[1]);
	});

	it('ends the current turn and its probes with nothing in its place', () => {
		const p = probes();
		const turns = searchTurns(() => p);
		const a = turns.take();
		turns.end();
		expect(a.current()).toBe(false);
		expect(p.end).toHaveBeenCalledTimes(1);
		turns.end();
		expect(p.end).toHaveBeenCalledTimes(1);
	});
});
