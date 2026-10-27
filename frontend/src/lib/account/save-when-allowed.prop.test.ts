import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	busyWaitSecs,
	DEFAULT_WAIT_S,
	MAX_SAVE_ATTEMPTS,
	MAX_WAIT_S,
	saveWhenAllowed,
	SupersededError,
	waitWhileWanted
} from './save-when-allowed';

/** An error of the shape the account api rejects with. */
const shaped = (status: number, detail: string) =>
	Object.assign(new Error('x'), { status, detail });

describe('busyWaitSecs, by property', () => {
	it('is a whole number of seconds within the cap for any 429', () => {
		fc.assert(
			fc.property(
				fc.oneof(fc.double({ noNaN: false }), fc.integer(), fc.constant(null)),
				(secs) => {
					const wait = busyWaitSecs(shaped(429, JSON.stringify({ retry_after_secs: secs })));
					expect(wait).not.toBeNull();
					expect(Number.isInteger(wait)).toBe(true);
					expect(wait!).toBeGreaterThanOrEqual(0);
					expect(wait!).toBeLessThanOrEqual(MAX_WAIT_S);
				}
			)
		);
	});

	it('honours an advertised wait, rounded up, up to the cap', () => {
		fc.assert(
			fc.property(fc.double({ min: 0, max: 10_000, noNaN: true }), (secs) => {
				const wait = busyWaitSecs(shaped(429, JSON.stringify({ retry_after_secs: secs })));
				expect(wait).toBe(Math.min(Math.ceil(secs), MAX_WAIT_S));
			})
		);
	});

	it('waits the default on a 429 whose wait is missing, negative or unreadable', () => {
		fc.assert(
			fc.property(
				fc.oneof(
					fc.string(),
					fc
						.double({ max: -Number.MIN_VALUE, noNaN: true })
						.map((n) => JSON.stringify({ retry_after_secs: n })),
					fc.constant(JSON.stringify({ retry_after_secs: 'soon' })),
					fc.constant('{}')
				),
				(body) => {
					let parsed: unknown;
					try {
						parsed = JSON.parse(body)?.retry_after_secs;
					} catch {
						parsed = undefined;
					}
					fc.pre(!(typeof parsed === 'number' && Number.isFinite(parsed) && parsed >= 0));
					expect(busyWaitSecs(shaped(429, body))).toBe(DEFAULT_WAIT_S);
				}
			)
		);
	});

	it('is no wait for anything that is not a 429', () => {
		fc.assert(
			fc.property(fc.integer({ min: 100, max: 599 }), fc.string(), (status, detail) => {
				fc.pre(status !== 429);
				expect(busyWaitSecs(shaped(status, detail))).toBeNull();
			})
		);
		fc.assert(
			fc.property(fc.anything(), (value) => {
				fc.pre(!(value instanceof Error));
				expect(busyWaitSecs(value)).toBeNull();
			})
		);
	});
});

describe('saveWhenAllowed, by property', () => {
	it('tries at most the cap, says each wait before it, and re-sends nothing once unwanted', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.integer({ min: 0, max: 8 }),
				fc.integer({ min: 1, max: 6 }),
				fc.integer({ min: 0, max: 8 }),
				async (refusals, maxAttempts, wantedFor) => {
					let attempts = 0;
					let notices = 0;
					let waits = 0;
					let asked = 0;
					const run = saveWhenAllowed(
						async () => {
							attempts += 1;
							if (attempts <= refusals) throw shaped(429, '{"retry_after_secs":0}');
							return 'saved';
						},
						{
							notify: () => {
								notices += 1;
							},
							wait: async () => {
								waits += 1;
							},
							maxAttempts,
							stillWanted: () => {
								asked += 1;
								return asked <= wantedFor;
							}
						}
					);
					const outcome = await run.then(
						(v) => v,
						(e: unknown) => e
					);
					expect(attempts).toBeLessThanOrEqual(maxAttempts);
					expect(notices).toBe(waits);
					// Every wait is followed by one more attempt, unless the write
					// was no longer wanted when it ended.
					expect(attempts).toBe(outcome instanceof SupersededError ? waits : waits + 1);
					if (outcome === 'saved') {
						expect(attempts).toBe(refusals + 1);
					} else if (outcome instanceof SupersededError) {
						expect(asked).toBe(wantedFor + 1);
					} else {
						expect(attempts).toBe(Math.min(maxAttempts, refusals));
					}
				}
			)
		);
	});

	it('never gives a write more attempts than the default cap', async () => {
		let attempts = 0;
		await saveWhenAllowed(
			async () => {
				attempts += 1;
				throw shaped(429, '{"retry_after_secs":0}');
			},
			{ notify: () => {}, wait: async () => {} }
		).catch(() => {});
		expect(attempts).toBe(MAX_SAVE_ATTEMPTS);
	});
});

describe('saveWhenAllowed once the write is no longer wanted, by property', () => {
	it('ends superseded, with no attempt after the wait at which it stopped being wanted', async () => {
		await fc.assert(
			fc.asyncProperty(fc.integer({ min: 0, max: 4 }), async (wantedFor) => {
				let attempts = 0;
				let asked = 0;
				const outcome = await saveWhenAllowed(
					async () => {
						attempts += 1;
						throw shaped(429, '{"retry_after_secs":0}');
					},
					{
						notify: () => {},
						wait: async () => {},
						maxAttempts: 10,
						stillWanted: () => {
							asked += 1;
							return asked <= wantedFor;
						}
					}
				).catch((e: unknown) => e);
				expect(outcome).toBeInstanceOf(SupersededError);
				// One first attempt, then one per wait while still wanted.
				expect(attempts).toBe(wantedFor + 1);
				expect(asked).toBe(wantedFor + 1);
			})
		);
	});
});

describe('waitWhileWanted, by property', () => {
	it('ends at the first look at or past its deadline or that finds it unwanted, however late each timer wakes', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.integer({ min: 0, max: 300_000 }),
				fc.integer({ min: 1, max: 5000 }),
				fc.array(fc.integer({ min: 0, max: 120_000 }), { maxLength: 50 }),
				fc.option(fc.integer({ min: 0, max: 60 }), { nil: undefined }),
				async (ms, step, lateness, unwantedFrom) => {
					let clock = 0;
					const looks: number[] = [];
					const timer: { fire: (() => void) | null } = { fire: null };
					const after = (d: number, tick: () => void) => {
						// Never asked to sleep past the deadline, nor longer than a step.
						expect(d).toBeGreaterThanOrEqual(0);
						expect(d).toBeLessThanOrEqual(step);
						expect(clock + d).toBeLessThanOrEqual(Math.max(ms, clock));
						const late = lateness[looks.length] ?? 0;
						timer.fire = () => {
							clock += d + late;
							looks.push(clock);
							tick();
						};
						return () => (timer.fire = null);
					};
					const unwanted = () => unwantedFrom !== undefined && looks.length > unwantedFrom;
					let done = false;
					void waitWhileWanted(ms, {
						after,
						now: () => clock,
						stillWanted: () => !unwanted(),
						checkEvery: step
					}).then(() => {
						done = true;
					});
					while (timer.fire) {
						const fire = timer.fire;
						timer.fire = null;
						fire();
					}
					await new Promise((r) => setTimeout(r, 0));
					expect(done).toBe(true);
					// The last look ended it; every look before it found the write
					// wanted and the deadline still ahead.
					const end = looks.length - 1;
					expect(looks[end] >= ms || unwantedFrom === end).toBe(true);
					for (let i = 0; i < end; i += 1) {
						expect(looks[i]).toBeLessThan(ms);
						expect(unwantedFrom !== undefined && i > unwantedFrom).toBe(false);
					}
				}
			)
		);
	});
});

describe('the tracker a held write names as busy, by property', () => {
	it('is the one the refusal names, when it names AniList or MyAnimeList, and no one otherwise', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.oneof(
					fc.constantFrom('anilist', 'mal'),
					fc.constantFrom('kitsu', 'inhouse', '', 'AniList'),
					fc.string(),
					fc.integer(),
					fc.constant(null),
					fc.constant(undefined)
				),
				async (refusedBy) => {
					const body = JSON.stringify({ retry_after_secs: 0, refused_by: refusedBy });
					const told: unknown[] = [];
					await saveWhenAllowed(
						async () => {
							if (told.length === 0) throw shaped(429, body);
							return 'saved';
						},
						{ notify: (_s, by) => told.push(by), wait: async () => {} }
					);
					const expected = refusedBy === 'anilist' || refusedBy === 'mal' ? refusedBy : null;
					expect(told).toEqual([expected]);
				}
			)
		);
	});
});
