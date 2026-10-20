import { it } from 'vitest';

import { API_BASE, useHandlers } from './helpers';

it('answered before teardown', async () => {
	useHandlers();
	await fetch(`${API_BASE}/ok`);
	await fetch(`${API_BASE}/slow`);
});
