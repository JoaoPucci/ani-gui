import { http } from 'msw';
import { it } from 'vitest';

import { API_BASE, server } from '../../acceptance/setup';
import { useHandlers, wait } from './helpers';

it('meets a handler that throws', async () => {
	useHandlers();
	server.use(
		http.get(`${API_BASE}/boom`, () => {
			throw new Error('the handler broke');
		})
	);
	await fetch(`${API_BASE}/boom`).catch(() => {});
	await wait(20);
});

it('runs after the handler threw', async () => {
	useHandlers();
	await wait(20);
});
