import { it } from 'vitest';

import { send, useHandlers, wait } from './helpers';

it('sends a request no handler answers', async () => {
	useHandlers();
	send('/nobody');
	await wait(20);
});

it('runs after the undescribed request', async () => {
	useHandlers();
	await wait(20);
});
