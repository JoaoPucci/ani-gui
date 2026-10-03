// @vitest-environment happy-dom
//
// The play page owns its video element: made when the page mounts,
// released when it leaves. What carries from one page to the next is
// the listener's volume and mute, as it did when one element served
// every page.
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	__resetPlayerAudioForTests,
	createPlayerVideo,
	placePlayerVideo,
	releasePlayerVideo
} from './player-video';

afterEach(() => {
	__resetPlayerAudioForTests();
	document.body.innerHTML = '';
});

describe('createPlayerVideo', () => {
	it('makes a CORS-enabled autoplaying element: every source and track it loads comes from the proxy origin', () => {
		const v = createPlayerVideo();
		expect(v.tagName).toBe('VIDEO');
		expect(v.getAttribute('crossorigin')).toBe('anonymous');
		expect(v.autoplay).toBe(true);
		expect(v.preload).toBe('auto');
	});

	it('makes a fresh element per page', () => {
		expect(createPlayerVideo()).not.toBe(createPlayerVideo());
	});

	it("carries the listener's volume and mute to the next page's element", () => {
		const first = createPlayerVideo();
		first.volume = 0.35;
		first.muted = true;
		first.dispatchEvent(new Event('volumechange'));
		const next = createPlayerVideo();
		expect(next.volume).toBeCloseTo(0.35);
		expect(next.muted).toBe(true);
	});
});

describe('placePlayerVideo', () => {
	it('puts the element in the slot, and a slot that holds it keeps it as it is', () => {
		const slot = document.createElement('div');
		const v = createPlayerVideo();
		expect(placePlayerVideo(v, slot)).toBe(v);
		expect(v.parentElement).toBe(slot);
		const append = vi.spyOn(slot, 'appendChild');
		placePlayerVideo(v, slot);
		expect(append).not.toHaveBeenCalled();
	});

	it('moves the element to a slot rendered again', () => {
		const first = document.createElement('div');
		const again = document.createElement('div');
		const v = createPlayerVideo();
		placePlayerVideo(v, first);
		placePlayerVideo(v, again);
		expect(v.parentElement).toBe(again);
		expect(first.children).toHaveLength(0);
	});
});

describe('releasePlayerVideo', () => {
	it('unloads the element and takes it out of the page', () => {
		const host = document.createElement('div');
		document.body.appendChild(host);
		const v = createPlayerVideo();
		host.appendChild(v);
		v.src = 'http://127.0.0.1:1/s/token/master.m3u8';
		const load = vi.spyOn(v, 'load');
		releasePlayerVideo(v);
		expect(v.hasAttribute('src')).toBe(false);
		expect(load).toHaveBeenCalled();
		expect(v.isConnected).toBe(false);
	});

	it('closes picture-in-picture when the element is the one in it', () => {
		const v = createPlayerVideo();
		document.body.appendChild(v);
		const exit = vi.fn(async () => {});
		Object.defineProperty(document, 'pictureInPictureElement', {
			configurable: true,
			get: () => v
		});
		Object.defineProperty(document, 'exitPictureInPicture', {
			configurable: true,
			value: exit
		});
		try {
			releasePlayerVideo(v);
			expect(exit).toHaveBeenCalledTimes(1);
		} finally {
			Reflect.deleteProperty(document, 'pictureInPictureElement');
			Reflect.deleteProperty(document, 'exitPictureInPicture');
		}
	});

	it('leaves another element in picture-in-picture alone', () => {
		const v = createPlayerVideo();
		const other = document.createElement('video');
		const exit = vi.fn(async () => {});
		Object.defineProperty(document, 'pictureInPictureElement', {
			configurable: true,
			get: () => other
		});
		Object.defineProperty(document, 'exitPictureInPicture', {
			configurable: true,
			value: exit
		});
		try {
			releasePlayerVideo(v);
			expect(exit).not.toHaveBeenCalled();
		} finally {
			Reflect.deleteProperty(document, 'pictureInPictureElement');
			Reflect.deleteProperty(document, 'exitPictureInPicture');
		}
	});
});
