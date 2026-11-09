import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { createFocusReturn, type Focusable } from './focus-return';

function control(connected = true) {
	const c = { isConnected: connected, focused: 0, focus: () => (c.focused += 1) };
	return c;
}

describe('createFocusReturn', () => {
	it('gives focus back to the control that held it', () => {
		const opener = control();
		const r = createFocusReturn();
		r.save(opener);
		r.restore();
		expect(opener.focused).toBe(1);
	});

	it('leaves focus alone when the control has gone', () => {
		const opener = control(false);
		const r = createFocusReturn();
		r.save(opener);
		r.restore();
		expect(opener.focused).toBe(0);
	});

	it('gives it back once: a second restore does nothing', () => {
		const opener = control();
		const r = createFocusReturn();
		r.save(opener);
		r.restore();
		r.restore();
		expect(opener.focused).toBe(1);
	});

	it('does nothing with nothing saved', () => {
		const r = createFocusReturn();
		expect(() => r.restore()).not.toThrow();
		r.save(null);
		expect(() => r.restore()).not.toThrow();
	});
});

describe('createFocusReturn, by property', () => {
	it('restores the last control saved, at most once, and only if connected', () => {
		fc.assert(
			fc.property(fc.array(fc.boolean(), { minLength: 1, maxLength: 5 }), (connected) => {
				const controls = connected.map((c) => control(c));
				const r = createFocusReturn();
				for (const c of controls) r.save(c as Focusable);
				r.restore();
				r.restore();
				const last = controls.at(-1)!;
				controls.forEach((c) => expect(c.focused).toBe(c === last && last.isConnected ? 1 : 0));
			})
		);
	});
});
