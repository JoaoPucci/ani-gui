import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { shortTitle } from './short-title';

describe('shortTitle', () => {
	const jojo = [
		'JoJo no Kimyou na Bouken: Stone Ocean Part 2',
		'JoJo no Kimyou na Bouken: Stone Ocean Part 3',
		'JoJo no Kimyou na Bouken (TV)'
	];

	it('drops the leading part another member shares', () => {
		expect(shortTitle(jojo[1], jojo)).toBe('Stone Ocean Part 3');
	});

	it('takes the longest shared part', () => {
		const bleach = [
			'BLEACH: Sennen Kessen-hen - Ketsubetsu-tan',
			'BLEACH: Sennen Kessen-hen - Soukoku-tan'
		];
		expect(shortTitle(bleach[0], bleach)).toBe('Ketsubetsu-tan');
	});

	it('drops a trailing part that another member starts with', () => {
		const run = [
			'Steel Ball Run: JoJo no Kimyou na Bouken',
			'JoJo no Kimyou na Bouken: Ougon no Kaze',
			'JoJo no Kimyou na Bouken (TV)'
		];
		expect(shortTitle(run[0], run)).toBe('Steel Ball Run');
	});

	it('keeps a trailing part no member starts with as a whole word', () => {
		const run = ['Steel Ball Run: JoJo', 'JoJoland: Part 9'];
		expect(shortTitle(run[0], run)).toBe(run[0]);
	});

	it('what it keeps is a part of the title, never the whole of a shared part', () => {
		const part = fc.stringMatching(/^[A-Za-z ]{1,8}$/);
		fc.assert(
			fc.property(part, part, part, (a, b, c) => {
				const title = `${a}: ${b}`;
				const got = shortTitle(title, [title, `${b}: ${c}`]);
				expect(title.includes(got)).toBe(true);
				expect(got.length).toBeGreaterThan(0);
			})
		);
	});

	it('keeps a title nobody shares a part of', () => {
		expect(shortTitle(jojo[2], jojo)).toBe(jojo[2]);
		expect(shortTitle('Gintama', ['Gintama', "Gintama'"])).toBe('Gintama');
	});

	it('never returns an empty title', () => {
		fc.assert(
			fc.property(fc.array(fc.string(), { maxLength: 4 }), fc.string(), (others, t) => {
				expect(shortTitle(t, [t, ...others]).length > 0 || t === '').toBe(true);
			})
		);
	});
});
