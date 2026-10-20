import { it } from 'vitest';

import { send, useHandlers, wait } from './helpers';

it('leaves a timer that sends later', () => {
	useHandlers();
	setTimeout(() => send('/ok'), 150);
});

it('sends an undescribed request while the timer sends', async () => {
	useHandlers();
	send('/nobody');
	await wait(300);
});

it('runs after both', async () => {
	useHandlers();
	await wait(20);
});
