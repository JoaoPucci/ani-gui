import { it } from 'vitest';

import { send, useHandlers, wait } from './helpers';

it('leaves a request nothing will answer', () => {
	useHandlers();
	send('/never');
});

it('runs after the unanswered request', async () => {
	useHandlers();
	await wait(50);
});
