import { beforeAll, it } from 'vitest';

import { send, useHandlers, wait } from './helpers';

beforeAll(() => {
	send('/nobody');
});

it('runs after a request sent from beforeAll', async () => {
	useHandlers();
	await wait(20);
});
