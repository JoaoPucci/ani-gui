// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { holdOutside } from './page-inert';

const byId = (id: string) => document.getElementById(id)!;
const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
	document.body.innerHTML = `
		<header id="header"></header>
		<main id="main">
			<section id="rail"></section>
			<div id="holder"><div id="modal"><button id="inside">x</button></div></div>
		</main>`;
});

afterEach(() => {
	holdOutside(null);
	document.body.innerHTML = '';
});

describe('holdOutside', () => {
	it('makes the siblings of the root and of each ancestor inert, up to the body', () => {
		holdOutside(byId('modal'));
		expect(byId('header').hasAttribute('inert')).toBe(true);
		expect(byId('rail').hasAttribute('inert')).toBe(true);
		for (const id of ['main', 'holder', 'modal', 'inside']) {
			expect(byId(id).hasAttribute('inert')).toBe(false);
		}
	});

	it('gives the page back', () => {
		holdOutside(byId('modal'));
		holdOutside(null);
		expect(document.querySelectorAll('[inert]')).toHaveLength(0);
	});

	it('keeps an inert the page set itself', () => {
		byId('rail').setAttribute('inert', '');
		holdOutside(byId('modal'));
		holdOutside(null);
		expect(byId('rail').hasAttribute('inert')).toBe(true);
	});

	it('holds what mounts beside the root later, and lets go of it after', async () => {
		holdOutside(byId('modal'));
		const late = document.createElement('section');
		byId('main').appendChild(late);
		await flush();
		expect(late.hasAttribute('inert')).toBe(true);
		holdOutside(null);
		const later = document.createElement('section');
		byId('main').appendChild(later);
		await flush();
		expect(late.hasAttribute('inert')).toBe(false);
		expect(later.hasAttribute('inert')).toBe(false);
	});

	it('moves the hold to a new root', () => {
		holdOutside(byId('modal'));
		holdOutside(byId('rail'));
		expect(byId('rail').hasAttribute('inert')).toBe(false);
		expect(byId('holder').hasAttribute('inert')).toBe(true);
		expect(byId('header').hasAttribute('inert')).toBe(true);
	});

	it('keeps the live elements and their way up within reach, and the rest of their containers inert', () => {
		document.body.innerHTML = `
			<header id="header"><span id="title"></span><button id="win">x</button></header>
			<main id="main">
				<section id="rail"></section>
				<section id="show"><p id="text"></p><div id="place"><a id="link">l</a></div></section>
				<div id="holder"><div id="modal"></div></div>
			</main>`;
		holdOutside(byId('modal'), [byId('place'), byId('win')]);
		for (const id of ['main', 'holder', 'modal', 'show', 'place', 'link', 'header', 'win']) {
			expect(byId(id).closest('[inert]'), id).toBeNull();
		}
		for (const id of ['title', 'rail', 'text']) {
			expect(byId(id).hasAttribute('inert'), id).toBe(true);
		}
		holdOutside(null);
		expect(document.querySelectorAll('[inert]')).toHaveLength(0);
	});

	it('holds what mounts beside a live element later', async () => {
		holdOutside(byId('modal'), [byId('rail')]);
		const late = document.createElement('p');
		byId('rail').parentElement!.appendChild(late);
		await flush();
		expect(late.hasAttribute('inert')).toBe(true);
		expect(byId('rail').hasAttribute('inert')).toBe(false);
	});

	it('leaves out a live element no longer on the page', () => {
		const gone = document.createElement('div');
		holdOutside(byId('modal'), [gone]);
		expect(byId('rail').hasAttribute('inert')).toBe(true);
	});
});
