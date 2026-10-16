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
});
