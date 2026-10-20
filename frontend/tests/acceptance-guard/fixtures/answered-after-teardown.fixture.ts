import { it } from 'vitest';

import { send, useHandlers, wait } from './helpers';

it('leaves a slow request behind', () => {
	useHandlers(150);
	send('/slow');
});

it('runs while the earlier request is answered', async () => {
	useHandlers();
	await wait(300);
});
