// The acceptance tier's own config, pointed at the guard's fixtures
// instead of its scenarios: the same environment, the same setup file
// and the same hook order, so a fixture meets the guard exactly as a
// scenario does. Run by guard.test.ts, never on its own.

import acceptance from '../../vitest.acceptance.config';

export default {
	...acceptance,
	test: {
		...acceptance.test,
		include: ['tests/acceptance-guard/fixtures/*.fixture.ts']
	}
};
