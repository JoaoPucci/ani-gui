import { it } from 'vitest';

import { send, useHandlers } from './helpers';

// A page that aborts what it no longer needs leaves nothing behind.
it('aborts its requests before teardown', () => {
	useHandlers();
	const slow = new AbortController();
	const never = new AbortController();
	send('/slow', { signal: slow.signal });
	send('/never', { signal: never.signal });
	slow.abort();
	never.abort();
});
