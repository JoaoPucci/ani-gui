import { it } from 'vitest';

import { send, useHandlers, wait } from './helpers';

it('sends an undescribed request and leaves a slow one behind', async () => {
	useHandlers(150);
	send('/nobody');
	await wait(20);
	send('/slow');
});

it('runs after both', async () => {
	useHandlers();
	await wait(300);
});
