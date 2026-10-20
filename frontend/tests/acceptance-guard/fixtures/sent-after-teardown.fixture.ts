import { it } from 'vitest';

import { send, useHandlers, wait } from './helpers';

it('leaves a timer that sends later', () => {
	useHandlers();
	setTimeout(() => send('/ok'), 150);
});

it('runs while the earlier timer sends', async () => {
	useHandlers();
	await wait(300);
});
