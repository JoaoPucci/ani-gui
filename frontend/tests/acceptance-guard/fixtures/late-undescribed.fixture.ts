import { it } from 'vitest';

import { send, useHandlers, wait } from './helpers';

it('leaves a timer that sends a request no handler answers', () => {
	useHandlers();
	setTimeout(() => send('/nobody'), 150);
});

it('runs while the earlier timer sends', async () => {
	useHandlers();
	await wait(300);
});
