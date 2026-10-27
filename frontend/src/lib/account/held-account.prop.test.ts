import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { hasLeft, standingOf } from './held-account';

const user = fc.oneof(
	fc.string({ minLength: 1, maxLength: 4 }),
	fc.constant(null),
	fc.constant(undefined)
);

describe('standingOf, by property', () => {
	it('is gone when either user is unknown, else same exactly when they are equal', () => {
		fc.assert(
			fc.property(user, user, (listed, now) => {
				const standing = standingOf(listed, now);
				if (listed == null || now == null) expect(standing).toBe('gone');
				else expect(standing).toBe(listed === now ? 'same' : 'other');
			})
		);
	});

	it('is symmetric', () => {
		fc.assert(
			fc.property(user, user, (a, b) => {
				expect(standingOf(a, b)).toBe(standingOf(b, a));
			})
		);
	});
});

describe('hasLeft, by property', () => {
	it('is final only for a disconnect or another user, never while a sign-in or a change is under way', () => {
		fc.assert(
			fc.property(
				user,
				fc.record({
					changing: fc.boolean(),
					connecting: fc.boolean(),
					disconnected: fc.boolean(),
					user
				}),
				(listed, now) => {
					const left = hasLeft(listed, now);
					if (listed == null) expect(left).toBe(true);
					else if (now.changing || now.connecting) expect(left).toBe(false);
					else expect(left).toBe(now.disconnected || now.user !== listed);
				}
			)
		);
	});
});
