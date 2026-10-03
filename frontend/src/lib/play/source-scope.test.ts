import { describe, expect, it } from 'vitest';
import { createSourceScope } from './source-scope';

describe('createSourceScope', () => {
	it('registrations accumulate and a flush runs then clears them', () => {
		const scope = createSourceScope();
		const ran: string[] = [];
		scope.add(() => ran.push('listeners'));
		scope.add(() => ran.push('engine'));
		scope.flush();
		expect(ran).toEqual(['listeners', 'engine']);
		scope.flush();
		expect(ran).toEqual(['listeners', 'engine']);
	});

	it('a cleanup registered during a flush waits for the next one', () => {
		const scope = createSourceScope();
		const ran: string[] = [];
		scope.add(() => {
			ran.push('first');
			scope.add(() => ran.push('late'));
		});
		scope.flush();
		expect(ran).toEqual(['first']);
		scope.flush();
		expect(ran).toEqual(['first', 'late']);
	});

	it('two scopes are two pages: flushing one leaves the other', () => {
		const a = createSourceScope();
		const b = createSourceScope();
		const ran: string[] = [];
		a.add(() => ran.push('a'));
		b.add(() => ran.push('b'));
		a.flush();
		expect(ran).toEqual(['a']);
	});
});
