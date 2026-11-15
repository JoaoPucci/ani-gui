import { afterEach, describe, expect, it } from 'vitest';
import * as secretModule from './secret';
import { SECRET_HEADER, secretHeaders, withSecretHeader } from './secret';

type WinHolder = { window?: { aniGui?: { internalSecret?: string | null } } };
const g = globalThis as unknown as WinHolder;

function bridge(internalSecret: string | null | undefined) {
	g.window = { aniGui: { internalSecret } };
}

afterEach(() => {
	delete g.window;
});

describe('the launch secret', () => {
	it('is sent as the header the backend reads', () => {
		expect(SECRET_HEADER).toBe('x-ani-gui-internal-secret');
	});

	it('becomes a header when the bridge carries it', () => {
		bridge('cafe');
		expect(secretHeaders()).toEqual({ 'x-ani-gui-internal-secret': 'cafe' });
	});

	it('is no header without a bridge or a secret', () => {
		expect(secretHeaders()).toEqual({});
		bridge(null);
		expect(secretHeaders()).toEqual({});
		bridge('');
		expect(secretHeaders()).toEqual({});
	});

	it('joins a request init, keeping its own headers and fields', () => {
		bridge('cafe');
		const signal = new AbortController().signal;
		expect(
			withSecretHeader({
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				signal
			})
		).toEqual({
			method: 'POST',
			headers: { 'content-type': 'application/json', 'x-ani-gui-internal-secret': 'cafe' },
			signal
		});
		expect(withSecretHeader()).toEqual({ headers: { 'x-ani-gui-internal-secret': 'cafe' } });
	});

	it('leaves a request init as it was without a secret', () => {
		const init = { method: 'DELETE' };
		expect(withSecretHeader(init)).toBe(init);
		expect(withSecretHeader()).toBeUndefined();
	});

	// A URL is kept where a header is not (Chromium's disk cache, log
	// lines): the routes an <img> or EventSource loads get the header
	// from the main process instead (electron/lib/secret-header.cjs).
	it('offers no way to put the secret in a URL', () => {
		expect(Object.keys(secretModule).sort()).toEqual([
			'SECRET_HEADER',
			'secretHeaders',
			'withSecretHeader'
		]);
	});
});
