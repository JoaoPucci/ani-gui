import { afterEach, describe, expect, it } from 'vitest';
import {
	SECRET_HEADER,
	SECRET_QUERY,
	secretHeaders,
	withSecretHeader,
	withSecretQuery
} from './secret';

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
		expect(SECRET_QUERY).toBe('secret');
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

	it('joins a URL as its last query parameter', () => {
		bridge('ca fe');
		expect(withSecretQuery('http://h/api/image?url=x')).toBe(
			'http://h/api/image?url=x&secret=ca%20fe'
		);
		expect(withSecretQuery('http://h/api/play/stream')).toBe(
			'http://h/api/play/stream?secret=ca%20fe'
		);
	});

	it('leaves a URL as it was without a secret', () => {
		expect(withSecretQuery('http://h/api/image?url=x')).toBe('http://h/api/image?url=x');
	});
});
