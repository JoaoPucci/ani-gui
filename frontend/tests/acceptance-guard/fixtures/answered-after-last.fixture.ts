import { it } from 'vitest';

import { send, useHandlers } from './helpers';

// The last scenario of its file: the answer lands in the file's grace.
it('leaves a slow request behind as the last scenario', () => {
	useHandlers(40);
	send('/slow');
});
