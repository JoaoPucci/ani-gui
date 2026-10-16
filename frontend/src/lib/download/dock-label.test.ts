import { describe, expect, it } from 'vitest';
import { dockLabel } from './dock-label';

describe('dockLabel', () => {
	it('counts the downloads in progress first', () => {
		expect(dockLabel([{ status: 'active' }, { status: 'active' }], 3)).toEqual({
			kind: 'active',
			count: 2
		});
	});

	it('falls back to the unseen ended rows, then to idle', () => {
		expect(dockLabel([], 3)).toEqual({ kind: 'unseen', count: 3 });
		expect(dockLabel([], 0)).toEqual({ kind: 'idle' });
	});

	it('counts the queued downloads apart from the running ones', () => {
		const active = [
			{ status: 'active' as const },
			{ status: 'active' as const },
			{ status: 'queued' as const }
		];
		expect(dockLabel(active, 0)).toEqual({ kind: 'active-queued', count: 2, queued: 1 });
	});
});
