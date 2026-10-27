import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { AccountApiError } from './api';
import { classifyRenewalError, isBearerRefused } from './renewal-failure';

/** A backend error as the account routes answer it: the HTTP status
 *  and the keyed envelope in the body. */
function answered(status: number, envelope: Record<string, unknown>): AccountApiError {
	return new AccountApiError(status, JSON.stringify(envelope));
}

const refused = answered(401, { kind: 'refresh_rejected', key: 'error.account.refresh_rejected' });

describe('classifyRenewalError', () => {
	it('reads the tracker refusing the refresh token as rejected', () => {
		expect(classifyRenewalError(refused)).toBe('rejected');
	});

	it.each([
		[
			'the tracker failing',
			answered(502, { kind: 'upstream', key: 'error.network.upstream', status: 503 })
		],
		[
			'the tracker busy',
			answered(429, { kind: 'upstream', key: 'error.network.upstream', status: 429 })
		],
		[
			'a rate limit',
			answered(429, {
				kind: 'rate_limited',
				key: 'error.network.rate_limited',
				retry_after_secs: 30
			})
		],
		[
			'no connection to the tracker',
			answered(503, { kind: 'network', key: 'error.network.unreachable' })
		],
		['the tracker timing out', answered(504, { kind: 'timeout', key: 'error.scraper.timeout' })]
	])('reads %s as unreachable', (_name, error) => {
		expect(classifyRenewalError(error)).toBe('unreachable');
	});

	it.each([
		[
			'an upstream status that is neither a refusal nor an outage',
			answered(502, { kind: 'upstream', key: 'error.network.upstream', status: 403 })
		],
		['a body that is not the envelope', new AccountApiError(500, 'Internal Server Error')],
		['an empty body', new AccountApiError(502, '')],
		[
			'a bearer the backend refused',
			answered(401, { kind: 'invalid_token', key: 'error.stream.invalid_token' })
		],
		['the local backend not answering', new TypeError('Failed to fetch')],
		['something that is not an error', 'nope']
	])('reads %s as unknown, never as a refusal or an outage', (_name, error) => {
		expect(classifyRenewalError(error)).toBe('unknown');
	});

	it('says rejected for the refusal key and nothing else, whatever the status', () => {
		// Only the backend's key says the token was refused: no status on
		// its own does, so a 401 or 400 with any other key is not one.
		fc.assert(
			fc.property(fc.integer({ min: 400, max: 599 }), fc.string(), (status, key) => {
				fc.pre(key !== 'error.account.refresh_rejected');
				const out = classifyRenewalError(answered(status, { kind: 'x', key }));
				return out !== 'rejected';
			})
		);
	});

	it('reads an upstream status as unreachable only when it is a 5xx or a 429', () => {
		fc.assert(
			fc.property(fc.integer({ min: 100, max: 599 }), (status) => {
				const out = classifyRenewalError(
					answered(502, { kind: 'upstream', key: 'error.network.upstream', status })
				);
				const outage = status >= 500 || status === 429;
				return out === (outage ? 'unreachable' : 'unknown');
			})
		);
	});
});

describe('isBearerRefused', () => {
	it('is true for a 401 carrying the refused-token key', () => {
		expect(
			isBearerRefused(answered(401, { kind: 'invalid_token', key: 'error.stream.invalid_token' }))
		).toBe(true);
	});

	it.each([
		['a refused refresh token', refused],
		['a 401 without the envelope', new AccountApiError(401, '')],
		['a 502', answered(502, { kind: 'upstream', key: 'error.network.upstream', status: 503 })],
		['a network failure', new TypeError('Failed to fetch')]
	])('is false for %s', (_name, error) => {
		expect(isBearerRefused(error)).toBe(false);
	});
});
