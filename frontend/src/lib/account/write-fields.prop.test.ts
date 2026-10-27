import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { coverFields, fieldsLeft, keepFields, keepWrite, type WriteField } from './write-fields';

const FIELDS: WriteField[] = ['status', 'progress'];
const fields = fc.subarray(FIELDS);
const body = fc.record(
	{
		kitsu_id: fc.constant('12'),
		status: fc.constantFrom('watching', 'paused', 'completed', 'dropped'),
		progress: fc.integer({ min: 0, max: 50 })
	},
	{ requiredKeys: ['kitsu_id'] }
);

describe('keepFields, by property', () => {
	it('sends a declared field only when it is left, and an undeclared one only beside a declared one', () => {
		fc.assert(
			fc.property(body, fields, fields, (b, declared, left) => {
				const kept = keepFields(b, declared, left);
				if (kept === null) return;
				const sent = FIELDS.filter((f) => kept[f] !== undefined);
				for (const f of sent) {
					if (declared.includes(f)) expect(left).toContain(f);
					else expect(kept[f]).toBe(b[f]);
				}
				expect(declared.some((f) => left.includes(f))).toBe(true);
				expect(sent.length).toBeGreaterThan(0);
				expect(kept.kitsu_id).toBe(b.kitsu_id);
			})
		);
	});

	it('keeps the whole body while every declared field is left', () => {
		fc.assert(
			fc.property(body, fields, (b, declared) => {
				fc.pre(declared.length > 0 && FIELDS.some((f) => b[f] !== undefined));
				expect(keepFields(b, declared, declared)).toEqual(b);
			})
		);
	});

	it('is nothing once no declared field is left', () => {
		fc.assert(
			fc.property(body, fields, (b, declared) => {
				expect(keepFields(b, declared, [])).toBeNull();
			})
		);
	});
});

describe('keepWrite, by property', () => {
	it('never sends a completion without its episode, and is keepFields otherwise', () => {
		fc.assert(
			fc.property(body, fields, fields, (b, declared, left) => {
				// Every completion the app writes carries its episode.
				fc.pre(b.status !== 'completed' || b.progress !== undefined);
				const kept = keepWrite(b, declared, left);
				if (kept?.status === 'completed') expect(kept.progress).not.toBeUndefined();
				const episodeGone = declared.includes('progress') && !left.includes('progress');
				if (b.status === 'completed' && episodeGone) expect(kept).toBeNull();
				else expect(kept).toEqual(keepFields(b, declared, left));
			})
		);
	});
});

describe('keepWrite and a completion whose episode was taken, by property', () => {
	it('drops the completion whole whenever its declared progress is not left', () => {
		fc.assert(
			fc.property(
				fc.nat(50),
				fc.constantFrom<WriteField[]>(['progress'], ['status', 'progress']),
				fc.constantFrom<WriteField[]>([], ['status']),
				(progress, declared, left) => {
					const finale = { kitsu_id: '12', status: 'completed', progress };
					expect(keepWrite(finale, declared, left)).toBeNull();
				}
			)
		);
	});
});

describe('the write ledger rules, by property', () => {
	/** Writes started in order, numbered from 1, each with its fields. */
	const writes = fc.array(fields, { minLength: 1, maxLength: 8 });

	it('leaves each write exactly the fields no later write covered', () => {
		fc.assert(
			fc.property(writes, (ws) => {
				const covered = ws.reduce(
					(c, fs, i) => coverFields(c, fs, i + 1),
					undefined as ReturnType<typeof coverFields> | undefined
				);
				ws.forEach((fs, i) => {
					const later = ws.slice(i + 1).flat();
					expect(fieldsLeft(covered, fs, i + 1)).toEqual(fs.filter((f) => !later.includes(f)));
				});
			})
		);
	});

	it('leaves the newest write all of its fields, and never a field it did not declare', () => {
		fc.assert(
			fc.property(writes, (ws) => {
				const covered = ws.reduce(
					(c, fs, i) => coverFields(c, fs, i + 1),
					undefined as ReturnType<typeof coverFields> | undefined
				);
				const n = ws.length;
				expect(fieldsLeft(covered, ws[n - 1], n)).toEqual(ws[n - 1]);
				ws.forEach((fs, i) => {
					for (const f of fieldsLeft(covered, FIELDS, i + 1)) expect(fs).toContain(f);
				});
			})
		);
	});

	it('does not change the coverage it was given', () => {
		fc.assert(
			fc.property(fields, fields, (a, b) => {
				const before = coverFields(undefined, a, 1);
				const snapshot = { ...before };
				coverFields(before, b, 2);
				expect(before).toEqual(snapshot);
			})
		);
	});
});
