/**
 * A keyboard focus ring is drawn once, around what the control looks
 * like, and whole.
 *
 * Every focusable control gets the app's ring on `:focus-visible`
 * (frontend/src/lib/design/tokens.css): two box shadows around the
 * element's own box. Some controls drew theirs wrong:
 *
 *   - The window's minimize, maximize and close buttons. Each is the
 *     topbar's full height, so the corner stays easy to hit, and shows
 *     its hover and focus as a square chip in the middle. The chip got
 *     its ring, and the button kept the app's ring as well: a tall
 *     rectangle around the whole click target, its top edge cut off by
 *     the top of the window.
 *   - The app's mark at the top of the rail, a link home. It drew an
 *     outline as a wide pill around the mark and its name, on top of
 *     the app's ring, and the window's edge cut the pill's side off.
 *   - The rail's links and its version link span the rail from the
 *     window's edge, so the window cut their rings' side off too.
 *   - The buttons of a segmented control (Sub/Dub, the settings'
 *     choices) sit inside a rounded track that clips what overflows
 *     it, and it cut their rings.
 *   - A control the keyboard scrolled into view stopped flush with the
 *     window's bottom edge, its ring past it; going back with
 *     Shift+Tab, one stopped under the sticky topbar, hidden.
 *   - A show's card drew the app's ring around the whole card and a
 *     second ring around its poster, and a card the keyboard scrolled
 *     into a row stopped flush with the row's edge, which cut its ring.
 *
 * Layout is the subject, so this runs in the packaged app's Chromium.
 * The keyboard moves focus through every stop on Home, a show's page,
 * Settings and a search's results, forward with Tab and back with
 * Shift+Tab: at the smallest window the app allows and at a common
 * desktop size, then maximized, at the window's own size. Home's row of
 * top-rated shows is long enough to scroll. At every stop the focused
 * control's rings are read from computed styles:
 *
 *   - no ring leaves the window or is cut by a box that clips what
 *     overflows it; for a control that scrolls with the page, the window
 *     starts under the sticky topbar. A ring drawn inside a control's box
 *     is taken as whole when the box is;
 *   - one ring is drawn: on the control, on its `::before` chip, or on
 *     one element inside it (a card's poster), never on two of them, and
 *     never as an outline and a ring shadow together;
 *   - a ring drawn inside a box shows against the box's fill, and the
 *     active rail link's rule does not cover it;
 *   - the controls this change is about each draw a ring, and each page
 *     reaches the ones it has;
 *   - the rail's mark rings the way the rail's links do, with as much
 *     room above the mark as below its name.
 *
 * A ring is an outline or a hard-edged shadow (a spread, no blur); a
 * soft shadow, a card's resting elevation, is not one. A ring on an
 * element inside the control is one a style rule draws there through a
 * `:focus-visible` in its selector, found through the page's style
 * sheets and the element's `matches`. A ring the control's focus draws
 * beside it or on an element hugging it (no edge more than 64px past
 * the control's), through `:focus-visible` or `:focus-within`, counts as
 * its ring; one on a larger element around it counts as a second ring
 * beside the control's own, and as its ring only when the control is
 * hidden. A control is hidden when it is transparent or has no more than
 * a pixel's box (a visually hidden submit button); a ring on a hidden
 * control does not count. A ring beside or around the control must show
 * against what it is drawn on.
 *
 * More tests walk what shows only in some states, at the desktop
 * size: Home with a guessed Continue Watching card, a pinned sign-in
 * toast and the update badge; the card's link dialog; a show's page
 * with a download in flight, which shows the download bar; the
 * player's page with the skip button up and the note on a guessed
 * session; a show's page with its watch-order strip (its tiles and
 * the line for the entry being watched); with the watch-order mockups
 * on, their franchise page (its rows and its play button), opened by
 * its address; and the player at the end of a show's last episode,
 * with the Up next card naming the entry after it. Each requires the
 * walk to reach those controls.
 *
 * A forced-colours theme (a Windows contrast theme) drops every box
 * shadow, so there a ring must be an outline. With the theme emulated,
 * a last test walks Home, a show's page (also with an episode typed),
 * Settings and Search at the desktop size, and the state tests walk their screens again: every
 * stop must show an outline the eye can see (its own or its chip's on
 * a visible control, or one its focus draws on an element inside,
 * beside or hugging it), whole (but for an outline drawn inside it),
 * and the window buttons only on their chip. The
 * smallest and the maximized windows are not walked in that theme.
 *
 * Home's hero with nothing to feature and with trending unanswered is
 * walked too, forward only, at the desktop size, under the mouse and in
 * forced colours; the player test types a number so the episode field's Go
 * button is reached.
 *
 * What it does not reach: the Account, About and Debug pages, and the
 * menus, dialogs and popovers other than the link dialog and the
 * download list. A ring on an element
 * beside the control that is much larger than it is not seen.
 * Of what clips, it models an ancestor's overflow and the window, not an
 * ancestor's rounded corners, clip-path or mask. Where the window cannot
 * be maximized (CI's Xvfb has no window manager), the maximized pass is
 * noted on the test and not walked. It walks with reduced motion on, so
 * a row glides to a card only on the path it does not take.
 *
 * Where a ring lies is computed from its outline and its shadows'
 * spread around the element's box, and a `::before` chip's box from its
 * size, centred on its control as the window buttons place it: a
 * pseudo-element has no box a script can read. Every API request is
 * stubbed, but for the player test's play and stream, which run on the
 * backend's replay of a seeded row (see real-play.ts).
 *
 * A ring on an element inside a control is found only through a rule
 * that sets its outline or shadow under `:focus-visible`: one drawn
 * through `:focus-within`, or by a rule that changes only an outline's
 * colour, width or offset, is not seen.
 *
 * Under the mouse: at the desktop size, the forward walks of the four
 * pages (and of the show's page with an episode typed), of the hero's
 * empty and failed states, of the player, of the watch-order strip, of
 * the mockups' franchise page and of the Up next card rest the mouse
 * on every stop that shows, and a hover style must neither replace its ring nor
 * add a second; the read must find the control under the mouse.
 * Whether a lifted, hovered control's ring stays whole is not checked,
 * and the other state tests (Continue Watching, its link dialog, the
 * download bar and list) do not rest the mouse.
 */
import { type CDPSession, type ElectronApplication, type Page } from '@playwright/test';
import { expect, test } from './policy-guard';
import { withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import http from 'node:http';
import type net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import {
	appInfo,
	continueHistory,
	continueKitsuEpisode6,
	continueKitsuMatch,
	defaultSettings,
	topRated,
	trending
} from './fixtures/kitsu';
import {
	appWindow,
	cardLinesAnswer,
	launchPackagedApp,
	packagedBinary,
	stopApp,
	stopDeadApp
} from './launch';
import { playFromContinue, routeRealPlay, writeReplayConfig } from './real-play';
import { answer } from './stubs';

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

/** Enough shows that a row of them overflows its strip at the widest
 *  size walked, so the keyboard scrolls the strip to reach its cards. */
const manyTopRated = Array.from({ length: 14 }, (_, i) =>
	i === 0 ? topRated[0] : { ...topRated[0], id: `${topRated[0].id}${i}` }
);

async function launchOnce(onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-focus-rings-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	const app = await launchPackagedApp(
		{
			...process.env,
			XDG_STATE_HOME: path.join(tmp, 'state'),
			XDG_CONFIG_HOME: path.join(tmp, 'config'),
			XDG_CACHE_HOME: path.join(tmp, 'cache'),
			XDG_DATA_HOME: path.join(tmp, 'data'),
			ANI_GUI_UNATTENDED: '1'
		},
		{ profile: tmp }
	);
	onLaunch(app);
	const page = await appWindow(app);

	await page.route('**/api/**', async (r) => {
		const p = new URL(r.request().url()).pathname;
		const j = (body: unknown) =>
			answer(r, {
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(body)
			});
		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') {
			return r.request().method() === 'PUT' ? answer(r, { status: 204 }) : j(defaultSettings);
		}
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j(manyTopRated);
		if (p.startsWith('/api/image')) return answer(r, { status: 503 });
		if (p === '/api/history') return j([]);
		if (p === '/api/history/card-lines') return j({});
		if (p === '/api/watched-at') return j({});
		if (p === '/api/availability/batch') {
			const { kitsu_ids } = r.request().postDataJSON() as {
				kitsu_ids: string[];
			};
			return j({
				cached: Object.fromEntries(kitsu_ids.map((id) => [id, true])),
				playable_episode_counts: {}
			});
		}
		if (p === '/api/availability/warm') return answer(r, { status: 202 });
		if (p === '/api/update-check') return answer(r, { status: 204 });
		if (p.startsWith('/api/kitsu/anime/')) return j(topRated[0]);
		if (p.startsWith('/api/kitsu/episodes/')) return j([]);
		if (p === '/api/download/default-dir') return j(null);
		if (p === '/api/availability') {
			return j({ available: true, episode_count: 12, extra_episodes: [] });
		}
		if (p === '/api/kitsu/search') return j([]);
		if (p === '/api/kitsu/search/page') return j({ hits: manyTopRated, next_offset: null });
		if (p.startsWith('/api/play/stream')) return answer(r, { status: 503 });
		return j(null);
	});

	const homeUrl = page.url();
	await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
	await page.goto('about:blank');
	await page.goto(homeUrl, { waitUntil: 'domcontentloaded' });
	return { app, page };
}

async function launch() {
	let pending: ElectronApplication | null = null;
	return withColdLaunchRetry(
		async () => {
			const launched = await launchOnce((app) => {
				pending = app;
			});
			pending = null;
			return launched;
		},
		{
			cleanup: async () => {
				const dead = pending;
				pending = null;
				if (dead) await stopDeadApp(dead);
			}
		}
	);
}

/** One DevTools session per page, kept open: an override lasts only as
 *  long as the session that set it. */
const sessions = new WeakMap<Page, CDPSession>();

/** Lays the page out at `width` × `height` (see home-pill-fit.spec.ts). */
async function sizeViewport(page: Page, width: number, height: number) {
	let cdp = sessions.get(page);
	if (!cdp) {
		cdp = await page.context().newCDPSession(page);
		sessions.set(page, cdp);
	}
	await cdp.send('Emulation.setDeviceMetricsOverride', {
		width,
		height,
		deviceScaleFactor: 0,
		mobile: false
	});
	await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(width);
}

/** Lays the page out at the window's own size again. */
async function clearViewport(page: Page) {
	const cdp = sessions.get(page);
	if (cdp) await cdp.send('Emulation.clearDeviceMetricsOverride');
	await page.evaluate(
		() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
	);
}

interface Box {
	left: number;
	right: number;
	top: number;
	bottom: number;
}
type Kind = 'winctl' | 'brand' | 'nav' | 'rail-foot' | 'seg' | 'other';
interface Clip {
	name: string;
	box: Box;
}
interface Stop {
	/** How the control reads in a failure message. */
	name: string;
	/** The control's own ring, or null when it draws none. */
	ring: Box | null;
	/** The ring its `::before` chip carries, or null. */
	chipRing: Box | null;
	/** Rings its focus draws on elements inside it (a card's poster),
	 *  each with what clips that element. */
	childRings: { name: string; ring: Box; clips: Clip[] }[];
	/** Whether it draws anything to show focus on its own box: an
	 *  outline, or a shadow, inset ones included. */
	drawn: boolean;
	/** Whether it draws both an outline and a ring shadow: two rings. */
	twice: boolean;
	/** Its own box: an inset ring is drawn inside it. */
	box: Box;
	/** For a ring drawn inside the box: how wide it is, and how far its
	 *  outermost band's colour is from what it is drawn on (0 to 765,
	 *  summed over RGB). */
	inset: { width: number; contrast: number } | null;
	/** The box of an absolutely placed `::before` rule (the active rail
	 *  link's), when there is one and no chip is centred on the control. */
	rule: Box | null;
	/** The room above and below what the control holds, inside its box. */
	room: { above: number; below: number } | null;
	/** Its own ring's look, to compare one control's with another's.
	 *  `outline` is the outline's style alone: its colour is the text's,
	 *  and differs between controls that draw no outline. */
	look: { outline: string; boxShadow: string; borderRadius: string };
	/** The boxes, the window's first, that clip what overflows them. */
	clips: Clip[];
	/** Which of the controls this change is about it is, if any. */
	kind: Kind;
	/** Whether an outline the eye can see marks it: its own or its
	 *  chip's, on a control that is itself visible, or one that its focus
	 *  draws on an element inside, beside or hugging it (a switch's
	 *  track). That
	 *  is the ring a forced-colours theme keeps, since it drops every box
	 *  shadow. */
	outlined: boolean;
	/** Whether the control itself shows: rendered, not transparent,
	 *  with more than a pixel's box. A ring on a hidden control is not
	 *  seen. */
	visible: boolean;
	/** Whether its focus draws a ring (an outline or a hard-edged shadow)
	 *  on a visible element beside it or hugging it: a switch's track, a
	 *  slider's frame. */
	relatedRinged: boolean;
	/** Those rings beside or around it, each with what clips that element
	 *  and how far its colour (its outline's, or its outermost hard
	 *  shadow's) is from what it is drawn on, 0 to 765 over RGB. */
	relatedRings: { name: string; ring: Box; clips: Clip[]; contrast: number }[];
	/** Rings its focus draws on a larger element around it, which do not
	 *  count as its ring but do count as a second one. */
	outerRings: string[];
	/** Where its own outline and its chip's outline lie, if it draws
	 *  them: what a forced-colours theme shows of its ring. */
	outlines: Box[];
	/** Whether the mouse is over it (`:hover`). */
	hovering: boolean;
	/** The selectors asked about that the control matches. */
	hits: string[];
}

/** The focused control's rings and what may clip them, read in one frame. */
function readStop(page: Page, selectors: readonly string[] = []): Promise<Stop | null> {
	return page.evaluate((selectors) => {
		const el = document.activeElement as HTMLElement | null;
		if (!el || el === document.body) return null;
		const px = (v: string) => Number.parseFloat(v) || 0;
		/** A computed box-shadow, one entry per shadow: its colour, the
		 *  four lengths, and whether it is inset. */
		const shadows = (cs: CSSStyleDeclaration) => {
			if (!cs.boxShadow || cs.boxShadow === 'none') return [];
			return cs.boxShadow.split(/,(?![^(]*\))/).map((shadow) => {
				const colour = shadow.match(/\b(rgba?|color|oklab|oklch|hsla?)\([^)]*\)/)?.[0] ?? '';
				const [x = 0, y = 0, blur = 0, spread = 0] = (
					shadow.replace(colour, '').match(/-?[\d.]+px/g) ?? []
				).map(px);
				return { colour, x, y, blur, spread, inset: /\binset\b/.test(shadow) };
			});
		};
		/** How far a computed style's outline and outer rings reach past
		 *  the box; null when it draws neither. A ring is a hard-edged
		 *  shadow: no blur, a spread. A soft shadow (a card's resting
		 *  elevation) is not one. */
		const reach = (cs: CSSStyleDeclaration): number | null => {
			let r: number | null = null;
			if (cs.outlineStyle !== 'none' && px(cs.outlineWidth) > 0) {
				r = Math.max(0, px(cs.outlineWidth) + px(cs.outlineOffset));
			}
			for (const s of shadows(cs)) {
				if (s.inset || s.blur > 0 || s.spread <= 0) continue;
				r = Math.max(r ?? 0, Math.max(Math.abs(s.x), Math.abs(s.y)) + s.spread);
			}
			return r;
		};
		const grow = (b: DOMRect | Box, by: number): Box => ({
			left: b.left - by,
			right: b.right + by,
			top: b.top - by,
			bottom: b.bottom + by
		});
		const topbar = document.querySelector('.topbar');
		/** What clips a ring drawn around `start`: the window (under the
		 *  sticky topbar for what scrolls with the page), then each box
		 *  above `start` that clips what overflows it. */
		const clipsFor = (start: HTMLElement): Clip[] => {
			let pinned = false;
			for (let a: HTMLElement | null = start; a; a = a.parentElement) {
				const position = getComputedStyle(a).position;
				if (position === 'fixed' || position === 'sticky') pinned = true;
			}
			const clips: Clip[] = [
				{
					name: pinned || !topbar ? 'the window' : 'the window, under the topbar',
					box: {
						left: 0,
						right: window.innerWidth,
						top: pinned || !topbar ? 0 : topbar.getBoundingClientRect().bottom,
						bottom: window.innerHeight
					}
				}
			];
			for (
				let a: HTMLElement | null = start.parentElement;
				a && a !== document.body;
				a = a.parentElement
			) {
				const s = getComputedStyle(a);
				if (s.overflowX === 'visible' && s.overflowY === 'visible') continue;
				const r = a.getBoundingClientRect();
				// The clip is the padding box: inside the borders.
				clips.push({
					name: `${a.tagName.toLowerCase()}.${[...a.classList].join('.')}`,
					box: {
						left: s.overflowX === 'visible' ? -Infinity : r.left + px(s.borderLeftWidth),
						right: s.overflowX === 'visible' ? Infinity : r.right - px(s.borderRightWidth),
						top: s.overflowY === 'visible' ? -Infinity : r.top + px(s.borderTopWidth),
						bottom: s.overflowY === 'visible' ? Infinity : r.bottom - px(s.borderBottomWidth)
					}
				});
			}
			return clips;
		};
		/** A colour as RGBA bytes, through a canvas, which reads every
		 *  colour syntax the computed style may give. */
		const ctx = document.createElement('canvas').getContext('2d', {
			willReadFrequently: true
		}) as CanvasRenderingContext2D;
		const rgba = (c: string) => {
			ctx.clearRect(0, 0, 1, 1);
			ctx.fillStyle = '#000';
			ctx.fillStyle = c;
			ctx.fillRect(0, 0, 1, 1);
			return [...ctx.getImageData(0, 0, 1, 1).data];
		};
		/** The colour an element is painted on, as RGB: its own background
		 *  and those behind it, composited until one is opaque. A
		 *  see-through fill is blended over what it sits on. */
		const ground = (start: HTMLElement) => {
			const layers: number[][] = [];
			for (let a: HTMLElement | null = start; a; a = a.parentElement) {
				const c = rgba(getComputedStyle(a).backgroundColor);
				if (c[3] > 0) layers.push(c);
				if (c[3] === 255) break;
			}
			let rgb = [0, 0, 0];
			for (const c of layers.reverse()) {
				const alpha = c[3] / 255;
				rgb = rgb.map((v, i) => c[i] * alpha + v * (1 - alpha));
			}
			return rgb;
		};

		const cs = getComputedStyle(el);
		const own = reach(cs);
		const rect = el.getBoundingClientRect();
		const outlined = cs.outlineStyle !== 'none' && px(cs.outlineWidth) > 0;
		const before = getComputedStyle(el, '::before');
		const hasBefore = before.content !== 'none';
		const chipReach = hasBefore ? reach(before) : null;
		let chipRing: Box | null = null;
		if (chipReach !== null) {
			const w = px(before.width);
			const h = px(before.height);
			const cx = (rect.left + rect.right) / 2;
			const cy = (rect.top + rect.bottom) / 2;
			chipRing = grow(
				{
					left: cx - w / 2,
					right: cx + w / 2,
					top: cy - h / 2,
					bottom: cy + h / 2
				},
				chipReach
			);
		}
		let rule: Box | null = null;
		if (hasBefore && chipReach === null && before.position === 'absolute') {
			const left = rect.left + px(cs.borderLeftWidth) + px(before.left);
			const top = rect.top + px(cs.borderTopWidth) + px(before.top);
			const w = px(before.width);
			const h = px(before.height);
			if (w > 0 && h > 0) rule = { left, right: left + w, top, bottom: top + h };
		}
		const insets = shadows(cs).filter((s) => s.inset && s.spread > 0);
		let inset: Stop['inset'] = null;
		if (insets.length > 0) {
			const edge = rgba(insets[0].colour);
			const on = ground(el);
			inset = {
				width: Math.max(...insets.map((s) => s.spread)),
				contrast: on.reduce((sum, v, i) => sum + Math.abs(edge[i] - v), 0)
			};
		}

		// Rings the control's focus draws on other elements: a style rule
		// whose selector names `:focus-visible` or `:focus-within` and
		// that sets an outline or a shadow matches the element, which is
		// not itself focused, and the element draws a ring. A rule that
		// only moves or grows it (a scrubber's thumb) draws none. Inside
		// the control such a ring is a child ring; beside it or around it
		// (a switch's track, a search field's frame) it is a related one.
		const focusRules: CSSStyleRule[] = [];
		const collect = (rules: CSSRuleList) => {
			for (const r of rules) {
				if (
					r instanceof CSSStyleRule &&
					(r.selectorText.includes(':focus-visible') || r.selectorText.includes(':focus-within')) &&
					(r.style.boxShadow !== '' || r.style.outlineStyle !== '')
				) {
					focusRules.push(r);
				}
				if ('cssRules' in r && (r as CSSGroupingRule).cssRules) {
					collect((r as CSSGroupingRule).cssRules);
				}
			}
		};
		for (const sheet of document.styleSheets) {
			try {
				collect(sheet.cssRules);
			} catch {
				// A sheet from another origin cannot be read.
			}
		}
		const lined = (s: CSSStyleDeclaration) => s.outlineStyle !== 'none' && px(s.outlineWidth) > 0;
		/** Whether an element shows at all: rendered, not transparent,
		 *  and with more than a pixel's box. A focused input a switch
		 *  hides is not, nor a visually hidden button. */
		const shows = (e: Element) => {
			const r = e.getBoundingClientRect();
			return (
				e.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) &&
				r.width > 1 &&
				r.height > 1
			);
		};
		// Rings its focus draws beside it or around it: on an element
		// under its parent, or on an ancestor, that a `:focus-visible` or
		// `:focus-within` rule matches. Only an element hugging the
		// control counts (no edge more than 64px past the control's), so
		// a large container's focus ring is not taken for the control's.
		const box0 = el.getBoundingClientRect();
		const hugs = (r: DOMRect) =>
			r.left >= box0.left - 64 &&
			r.right <= box0.right + 64 &&
			r.top >= box0.top - 64 &&
			r.bottom <= box0.bottom + 64;
		let relatedOutlined = false;
		const relatedRings: Stop['relatedRings'] = [];
		const outerRings: string[] = [];
		const parent = el.parentElement;
		const ancestors: HTMLElement[] = [];
		for (let a = parent; a && a !== document.body; a = a.parentElement) ancestors.push(a);
		const near = [...ancestors, ...(parent?.querySelectorAll<HTMLElement>('*') ?? [])];
		for (const d of near) {
			if (d === el || el.contains(d)) continue;
			if (!focusRules.some((r) => d.matches(r.selectorText))) continue;
			if (!shows(d)) continue;
			const dr = d.getBoundingClientRect();
			const ds = getComputedStyle(d);
			const spread = shadows(ds).filter((s) => s.spread > 0 && s.blur === 0);
			if (!lined(ds) && spread.length === 0) continue;
			const label = `${d.tagName.toLowerCase()}.${[...d.classList].join('.')}`;
			if (!hugs(dr)) {
				if (d.contains(el)) outerRings.push(label);
				continue;
			}
			if (lined(ds)) relatedOutlined = true;
			const by = reach(ds);
			const on = ground(d.parentElement ?? d);
			const edge = lined(ds)
				? rgba(ds.outlineColor)
				: spread.length > 0
					? rgba(spread[spread.length - 1].colour)
					: null;
			relatedRings.push({
				name: `${d.tagName.toLowerCase()}.${[...d.classList].join('.')}`,
				ring:
					by === null
						? { left: dr.left, right: dr.right, top: dr.top, bottom: dr.bottom }
						: grow(dr, by),
				clips: clipsFor(d),
				contrast: edge
					? on.reduce((sum, v, i) => sum + Math.abs(edge[i] - v) * (edge[3] / 255), 0)
					: 0
			});
		}
		const relatedRinged = relatedRings.length > 0;
		// An outline its focus draws inside it, on an element that shows.
		let childOutlined = false;
		for (const d of el.querySelectorAll<HTMLElement>('*')) {
			if (!focusRules.some((r) => d.matches(r.selectorText))) continue;
			if (lined(getComputedStyle(d)) && shows(d)) childOutlined = true;
		}
		const childRings: Stop['childRings'] = [];
		for (const d of el.querySelectorAll<HTMLElement>('*')) {
			if (!focusRules.some((r) => d.matches(r.selectorText))) continue;
			const by = reach(getComputedStyle(d));
			if (by === null) continue;
			childRings.push({
				name: `${d.tagName.toLowerCase()}.${[...d.classList].join('.')}`,
				ring: grow(d.getBoundingClientRect(), by),
				clips: clipsFor(d)
			});
		}

		// The room above and below what the control holds.
		let room: Stop['room'] = null;
		const kids = [...el.children].map((c) => c.getBoundingClientRect()).filter((r) => r.height);
		if (kids.length > 0) {
			room = {
				above: Math.min(...kids.map((r) => r.top)) - rect.top,
				below: rect.bottom - Math.max(...kids.map((r) => r.bottom))
			};
		}

		const label =
			el.getAttribute('aria-label') ?? el.textContent?.trim().slice(0, 40) ?? el.tagName;
		const kind: Kind = el.classList.contains('winctl-btn')
			? 'winctl'
			: el.classList.contains('brand')
				? 'brand'
				: el.classList.contains('nav-link')
					? 'nav'
					: el.classList.contains('rail-foot')
						? 'rail-foot'
						: el.classList.contains('seg-btn')
							? 'seg'
							: 'other';
		return {
			name: `${el.tagName.toLowerCase()}.${[...el.classList].join('.')} "${label}"`,
			ring: own === null ? null : grow(rect, own),
			chipRing,
			childRings,
			look: {
				outline: cs.outlineStyle,
				boxShadow: cs.boxShadow,
				borderRadius: cs.borderRadius
			},
			clips: clipsFor(el),
			drawn: outlined || (cs.boxShadow !== '' && cs.boxShadow !== 'none'),
			twice: outlined && shadows(cs).some((s) => !s.inset && s.blur === 0 && s.spread > 0),
			box: {
				left: rect.left,
				right: rect.right,
				top: rect.top,
				bottom: rect.bottom
			},
			inset,
			rule,
			room,
			kind,
			outlined:
				((outlined || (hasBefore && lined(before))) && shows(el)) ||
				relatedOutlined ||
				childOutlined,
			visible: shows(el),
			relatedRinged,
			relatedRings,
			outerRings,
			outlines: [
				...(outlined ? [grow(rect, px(cs.outlineWidth) + px(cs.outlineOffset))] : []),
				...(hasBefore && lined(before)
					? [
							grow(
								{
									left: (rect.left + rect.right) / 2 - px(before.width) / 2,
									right: (rect.left + rect.right) / 2 + px(before.width) / 2,
									top: (rect.top + rect.bottom) / 2 - px(before.height) / 2,
									bottom: (rect.top + rect.bottom) / 2 + px(before.height) / 2
								},
								px(before.outlineWidth) + px(before.outlineOffset)
							)
						]
					: [])
			],
			hovering: el.matches(':hover'),
			hits: selectors.filter((s) => el.matches(s))
		};
	}, selectors);
}

/** Resolves once nothing that scrolls the focused control has moved
 *  for three frames: a row scrolls a card into view smoothly. */
function settled(page: Page): Promise<void> {
	return page.evaluate(
		() =>
			new Promise<void>((done) => {
				let last = '';
				let still = 0;
				let frames = 0;
				const tick = () => {
					const parts: number[] = [window.scrollX, window.scrollY];
					for (let a = document.activeElement; a; a = a.parentElement) {
						parts.push(a.scrollLeft, a.scrollTop);
					}
					const now = parts.join(',');
					still = now === last ? still + 1 : 0;
					last = now;
					if (still >= 3 || ++frames > 120) done();
					else requestAnimationFrame(tick);
				};
				requestAnimationFrame(tick);
			})
	);
}

/** Moves focus with `key` until it comes round to where it began, and
 *  reads every stop, noting which of `selectors` each matches. The
 *  page's first Tab enters it at the top; its first Shift+Tab, at the
 *  bottom. Inside a dialog (`fresh: false`) the walk starts from where
 *  focus is, since a click outside would close it. */
async function walk(
	page: Page,
	key: 'Tab' | 'Shift+Tab' | 'ArrowDown' | 'ArrowUp',
	{
		selectors = [],
		fresh = true,
		hovered
	}: { selectors?: readonly string[]; fresh?: boolean; hovered?: Stop[] } = {}
): Promise<Stop[]> {
	if (fresh) {
		await page.mouse.click(1, 1);
		await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
	}
	const stops: Stop[] = [];
	const seen = new Set<string>();
	let wrapped = false;
	// Inside a dialog or a popover the walk starts on a control already
	// focused, which a key press may carry out of it: that one is read
	// first.
	if (!fresh) {
		const id = await page.evaluate(() => {
			const el = document.activeElement as HTMLElement | null;
			if (!el || el === document.body) return null;
			el.dataset.focusWalk ??= String(Math.random());
			return el.dataset.focusWalk;
		});
		const stop = id === null ? null : await readStop(page, selectors);
		if (id !== null && stop) {
			seen.add(id);
			stops.push(stop);
		}
	}
	for (let i = 0; i < 400; i++) {
		await page.keyboard.press(key);
		await settled(page);
		const id = await page.evaluate(() => {
			const el = document.activeElement as HTMLElement | null;
			if (!el || el === document.body) return null;
			el.dataset.focusWalk ??= String(Math.random());
			return el.dataset.focusWalk;
		});
		if (id === null) continue;
		if (seen.has(id)) {
			wrapped = true;
			break;
		}
		seen.add(id);
		const stop = await readStop(page, selectors);
		if (stop) stops.push(stop);
		if (hovered && stop?.visible) {
			const { box } = stop;
			await page.mouse.move((box.left + box.right) / 2, (box.top + box.bottom) / 2);
			await settled(page);
			const under = await readStop(page, selectors);
			if (under) hovered.push(under);
			await page.mouse.move(1, 1);
		}
	}
	expect(wrapped, `${key} came round to where it began`).toBe(true);
	return stops;
}

const inside = (inner: Box, outer: Box) =>
	inner.left >= outer.left - 0.5 &&
	inner.right <= outer.right + 0.5 &&
	inner.top >= outer.top - 0.5 &&
	inner.bottom <= outer.bottom + 0.5;

/** Whether a stop shows a ring the eye can see: around itself, inside
 *  its box or on its chip when it is itself visible, or on an element
 *  inside, beside or hugging it; for a hidden control, also on a larger
 *  element around it. A soft shadow alone (a resting elevation) is not
 *  a ring. */
const ringed = (s: Stop) =>
	(s.visible && (s.ring !== null || s.inset !== null || s.chipRing !== null)) ||
	s.childRings.length > 0 ||
	s.relatedRinged ||
	(!s.visible && s.outerRings.length > 0);

function assertStops(
	stops: Stop[],
	at: string,
	required: readonly Kind[],
	need: readonly string[] = [],
	least = 4
) {
	expect(
		stops.length,
		`${at}: the keyboard reached controls (${stops.map((s) => s.name).join(' | ')})`
	).toBeGreaterThanOrEqual(least);
	const kinds = new Set(stops.map((s) => s.kind));
	for (const k of required) {
		expect.soft(kinds.has(k), `${at}: the walk reached a ${k} control`).toBe(true);
	}
	for (const sel of need) {
		expect
			.soft(
				stops.some((s) => s.hits.includes(sel)),
				`${at}: the walk reached ${sel} (${stops.map((s) => s.name).join(' | ')})`
			)
			.toBe(true);
	}
	for (const s of stops) {
		const where = `${at}: ${s.name} ${JSON.stringify(s)}`;
		// Whole: inside the window and every box that clips it. A ring
		// drawn inside the box is whole when the box is.
		// A hidden control's own ring is not seen, so it is not held to it.
		const rings: { ring: Box; clips: Clip[] }[] = [];
		const ownRing = s.ring ?? (s.drawn ? s.box : null);
		if (ownRing && s.visible) rings.push({ ring: ownRing, clips: s.clips });
		if (s.chipRing && s.visible) rings.push({ ring: s.chipRing, clips: s.clips });
		rings.push(...s.childRings, ...s.relatedRings);
		for (const { ring, clips } of rings) {
			for (const c of clips) {
				expect.soft(inside(ring, c.box), `${where}: its ring is cut by ${c.name}`).toBe(true);
			}
		}
		// Drawn once: one ring, on the control, its chip, or one element
		// inside it, and not an outline and a ring shadow together.
		expect.soft(s.twice, `${where}: draws an outline and a ring shadow`).toBe(false);
		if (s.chipRing) {
			expect.soft(s.ring, `${where}: draws a ring beside its chip's`).toBeNull();
		}
		if (s.childRings.length > 0) {
			expect.soft(s.drawn, `${where}: draws a ring beside the one inside it`).toBe(false);
			expect.soft(s.childRings.length, `${where}: draws one ring inside it`).toBe(1);
		}
		if (s.relatedRinged || s.outerRings.length > 0) {
			expect
				.soft(
					s.visible && (s.ring !== null || s.inset !== null || s.chipRing !== null),
					`${where}: draws a ring beside the one around or beside it`
				)
				.toBe(false);
			for (const r of s.relatedRings) {
				expect.soft(r.contrast, `${where}: the ring on ${r.name} shows`).toBeGreaterThan(60);
			}
		}
		if (s.kind === 'winctl') {
			expect.soft(s.chipRing, `${where}: its chip carries the ring`).not.toBeNull();
		}
		// Every control the keyboard reaches shows a ring the eye can see.
		expect.soft(ringed(s), `${where}: shows a ring`).toBe(true);
		// A ring drawn inside the box shows against what it is drawn on,
		// and nothing the control places inside its box covers it.
		if (s.inset) {
			expect.soft(s.inset.contrast, `${where}: its ring shows on its fill`).toBeGreaterThan(60);
			if (s.rule) {
				const within: Box = {
					left: s.box.left + s.inset.width,
					right: s.box.right - s.inset.width,
					top: s.box.top + s.inset.width,
					bottom: s.box.bottom - s.inset.width
				};
				expect.soft(inside(s.rule, within), `${where}: its rule covers its ring`).toBe(true);
			}
		}
		// The rail's mark sits evenly inside its ring.
		if (s.kind === 'brand' && s.room) {
			expect
				.soft(Math.abs(s.room.above - s.room.below), `${where}: the mark sits evenly in its ring`)
				.toBeLessThanOrEqual(1);
		}
	}
	// The rail's mark rings as the rail's links do.
	const nav = stops.find((s) => s.kind === 'nav');
	const brand = stops.find((s) => s.kind === 'brand');
	if (nav && brand)
		expect.soft(brand.look, `${at}: the mark's ring looks like a rail link's`).toEqual(nav.look);
}

const views = [
	{ label: 'smallest', width: 960, height: 600 },
	{ label: 'desktop', width: 1440, height: 900 }
] as const;

/** Under the mouse, a hover style must neither replace a control's
 *  ring nor add a second one. */
function assertHovered(stops: Stop[], at: string) {
	for (const s of stops) {
		const where = `${at}: ${s.name} ${JSON.stringify(s)}`;
		expect.soft(s.hovering, `${where}: is under the mouse`).toBe(true);
		expect.soft(ringed(s), `${where}: shows a ring`).toBe(true);
		expect.soft(s.twice, `${where}: draws an outline and a ring shadow`).toBe(false);
	}
}

/** In a forced-colours theme, which drops every box shadow: every stop
 *  shows an outline, whole, and a window button only on its chip. */
function assertForced(
	stops: Stop[],
	at: string,
	required: readonly Kind[],
	need: readonly string[] = [],
	least = 4
) {
	expect(stops.length, `${at}: the keyboard reached controls`).toBeGreaterThanOrEqual(least);
	const kinds = new Set(stops.map((s) => s.kind));
	for (const k of required) {
		expect.soft(kinds.has(k), `${at}: the walk reached a ${k} control`).toBe(true);
	}
	for (const sel of need) {
		expect
			.soft(
				stops.some((s) => s.hits.includes(sel)),
				`${at}: the walk reached ${sel}`
			)
			.toBe(true);
	}
	for (const s of stops) {
		const where = `${at}: ${s.name} ${JSON.stringify(s)}`;
		expect.soft(s.outlined, `${where}: draws an outline`).toBe(true);
		const outlines: { ring: Box; clips: Clip[] }[] = [
			...(s.visible ? s.outlines.map((ring) => ({ ring, clips: s.clips })) : []),
			...s.relatedRings
		];
		for (const { ring, clips } of outlines) {
			for (const c of clips) {
				expect.soft(inside(ring, c.box), `${where}: its outline is cut by ${c.name}`).toBe(true);
			}
		}
		if (s.kind === 'winctl') {
			expect.soft(s.outlines.length, `${where}: only its chip draws an outline`).toBe(1);
			expect.soft(s.look.outline, `${where}: the button draws none`).toBe('none');
		}
		// The theme is in force: a rail link's own ring, a shadow, is gone.
		if (s.kind === 'nav') {
			expect.soft(s.look.boxShadow, `${where}: the theme dropped its shadow`).toBe('none');
		}
	}
}

/** Emulates a forced-colours theme, or none. */
async function forcedColours(page: Page, on: boolean) {
	await page.emulateMedia({ reducedMotion: 'reduce', forcedColors: on ? 'active' : 'none' });
	expect(await page.evaluate(() => matchMedia('(forced-colors: active)').matches)).toBe(on);
}

/** The pages every walk covers, each with how to reach it from Home
 *  and the controls it must reach. */
function pagesOf(page: Page) {
	return [
		{
			label: 'home',
			need: [],
			required: ['winctl', 'brand', 'nav', 'rail-foot'],
			go: async () => {
				await expect(page.getByText(/top rated/i).first()).toBeVisible({
					timeout: 15_000
				});
			}
		},
		{
			label: 'show',
			need: [],
			required: ['winctl', 'brand', 'nav', 'rail-foot', 'seg'],
			go: async () => {
				await page.locator(`a[href$="/anime/${topRated[0].id}"]`).first().click();
				await expect.poll(() => page.url()).toContain(`/anime/${topRated[0].id}`);
				await expect(page.locator('button.ep-tile').first()).toBeVisible({
					timeout: 15_000
				});
			}
		},
		{
			// A number typed into the episode field turns on its Go button,
			// which sits inside the field's pill.
			label: 'show, an episode typed',
			need: ['.ep-jump-go'],
			required: ['winctl', 'brand', 'nav', 'rail-foot', 'seg'],
			go: async () => {
				await page.locator(`a[href$="/anime/${topRated[0].id}"]`).first().click();
				await expect.poll(() => page.url()).toContain(`/anime/${topRated[0].id}`);
				const field = page.locator('input.jump-input').first();
				await expect(field).toBeVisible({ timeout: 15_000 });
				await field.fill('3');
				await expect(page.locator('.ep-jump-go').first()).toBeEnabled();
			}
		},
		{
			label: 'settings',
			need: [],
			required: ['winctl', 'brand', 'nav', 'rail-foot', 'seg'],
			go: async () => {
				await page.locator('a.nav-link[href$="/settings"]').click();
				await expect.poll(() => page.url()).toContain('/settings');
				await expect(page.locator('select.select-input').first()).toBeVisible({
					timeout: 15_000
				});
			}
		},
		{
			label: 'search',
			need: [],
			required: ['winctl', 'brand', 'nav', 'rail-foot'],
			go: async () => {
				await expect(page.getByText(/top rated/i).first()).toBeVisible({ timeout: 15_000 });
				const input = page.locator('input[type="search"]').first();
				await input.fill('fullmetal');
				await input.press('Enter');
				await expect.poll(() => page.url()).toContain('/search');
				await expect(page.locator('a.card-link').first()).toBeVisible({
					timeout: 15_000
				});
			}
		}
	] as const;
}

test('every focus ring is drawn once and whole', async () => {
	test.setTimeout(240_000);
	const testInfo = test.info();
	const { app, page } = await launch();
	try {
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await expect(page.locator('.winctl-btn')).toHaveCount(3, {
			timeout: 15_000
		});
		const pages = pagesOf(page);
		for (const maximized of [false, true]) {
			await app.evaluate(({ BrowserWindow }, max) => {
				const win = BrowserWindow.getAllWindows().find((w) => !w.isDestroyed() && w.isVisible());
				if (max) win?.maximize();
				else win?.unmaximize();
			}, maximized);
			// The button the window shows for its state: Restore when
			// maximized. A display with no window manager (CI's Xvfb) cannot
			// maximize, and that pass is then said, not walked.
			const state = page.locator('.winctl-btn').nth(1);
			const label = maximized ? /restore/i : /maximize/i;
			const reached = await expect
				.poll(() => state.getAttribute('aria-label'), { timeout: 5_000 })
				.toMatch(label)
				.then(() => true)
				.catch(() => false);
			if (!reached) {
				testInfo.annotations.push({
					type: 'skipped',
					description: `the window could not be ${maximized ? 'maximized' : 'restored'} here`
				});
				continue;
			}
			// Maximized, the page is laid out at the window's own size, with
			// no size set from DevTools.
			const sizes = maximized ? [{ label: 'own-size', width: 0, height: 0 }] : views;
			for (const view of sizes) {
				if (maximized) await clearViewport(page);
				else await sizeViewport(page, view.width, view.height);
				testInfo.annotations.push({
					type: 'topbar',
					description: `${view.label}${maximized ? ', maximized' : ''}: ${await page.evaluate(
						() =>
							`window ${window.innerWidth}x${window.innerHeight}, topbar ${document.querySelector('.topbar')?.getBoundingClientRect().height}px`
					)}`
				});
				for (const p of pages) {
					await page.locator('a.brand').click();
					await p.go();
					// Settled: the route's content is on show, so the focus reset
					// a navigation makes has happened, and two frames have drawn.
					await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
					await page.evaluate(
						() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
					);
					const at = `${p.label}, ${view.label}${maximized ? ', maximized' : ''}`;
					for (const key of ['Tab', 'Shift+Tab'] as const) {
						// At the desktop size the walk forward also rests the
						// mouse on every stop: a hover style must not replace
						// or double the ring.
						const hovered: Stop[] = [];
						const underMouse = !maximized && view.label === 'desktop' && key === 'Tab';
						const stops = await walk(page, key, {
							selectors: p.need,
							hovered: underMouse ? hovered : undefined
						});
						testInfo.annotations.push({
							type: 'stops',
							description: `${at}, ${key}: ${stops.map((s) => s.name).join(' | ')}`
						});
						assertStops(stops, `${at}, ${key}`, p.required, p.need);
						if (underMouse) assertHovered(hovered, `${at}, under the mouse`);
					}
					if (p.label === 'home') await shoot(page, `${view.label}${maximized ? '-max' : ''}`);
				}
			}
		}
	} finally {
		await stopApp(app);
	}
});

/** Shots of the window buttons and the rail's mark, each focused from
 *  the keyboard. */
async function shoot(page: Page, suffix: string) {
	const testInfo = test.info();
	const focusByKeys = async (selector: string) => {
		await page.mouse.click(1, 1);
		await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
		for (let i = 0; i < 400; i++) {
			await page.keyboard.press('Tab');
			if (await page.evaluate((s) => document.activeElement?.matches(s) ?? false, selector)) {
				return;
			}
		}
		throw new Error(`the keyboard never reached ${selector}`);
	};
	const width = await page.evaluate(() => window.innerWidth);
	for (const [i, label] of ['minimize', 'maximize', 'close'].entries()) {
		await focusByKeys(`.winctl-btn:nth-child(${i + 1})`);
		await page.screenshot({
			path: testInfo.outputPath(`winctl-${label}-${suffix}.png`),
			clip: { x: width - 220, y: 0, width: 220, height: 80 }
		});
	}
	await focusByKeys('a.brand');
	await page.screenshot({
		path: testInfo.outputPath(`brand-${suffix}.png`),
		clip: { x: 0, y: 0, width: 160, height: 200 }
	});
}

/** One row, which Continue Watching guesses: it records no show. */
const guessedHistory = [{ id: 'allmanga-test-show', ep_no: '5', title: 'Continue Test Show' }];

const continueEpisodes = Array.from({ length: 12 }, (_, i) => ({
	...continueKitsuEpisode6,
	id: `ep-${i + 1}`,
	number: i + 1,
	relative_number: i + 1,
	canonical_title: `Episode ${i + 1}`
}));

/** A message from the shipped English catalogue. */
function message(file: string, key: string): string {
	const p = path.resolve(__dirname, '../../frontend/messages/en', `${file}.json`);
	return JSON.parse(fs.readFileSync(p, 'utf8'))[key] as string;
}

/** Mounts the first page again, so it asks through the stubs in place
 *  now (see home-continue.spec.ts). */
async function remount(page: Page) {
	const url = page.url();
	await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
	await page.goto('about:blank');
	await page.goto(url, { waitUntil: 'domcontentloaded' });
}

/** Replaces the page's EventSource with one that stays open and says
 *  nothing: a download started then stays in flight. */
async function silentStreams(page: Page) {
	await page.evaluate(() => {
		class SilentSource {
			readonly url: string;
			readyState = 1;
			constructor(url: string) {
				this.url = url;
			}
			addEventListener() {}
			removeEventListener() {}
			close() {
				this.readyState = 2;
			}
		}
		(window as unknown as { EventSource: unknown }).EventSource = SilentSource;
	});
}

test('the rings on Continue Watching, its link dialog, a toast, the update badge and the download bar are drawn once and whole', async () => {
	test.setTimeout(240_000);
	const { app, page } = await launch();
	try {
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await sizeViewport(page, 1440, 900);
		// A sign-in the keyring holds that ran out with no way to renew
		// it: the app pins a toast that offers to sign in again.
		await app.evaluate(({ ipcMain }) => {
			ipcMain.removeHandler('ani-gui:account:get-token');
			ipcMain.handle('ani-gui:account:get-token', (_e, provider: string) =>
				provider === 'anilist'
					? {
							ok: true,
							payload: {
								access_token: 'expired',
								refresh_token: null,
								expires_at_epoch_s: 1,
								user_id: '1',
								username: 'tester',
								avatar_url: null
							}
						}
					: { ok: false, kind: 'not_found' }
			);
		});
		await page.route('**/api/**', async (r) => {
			const p = new URL(r.request().url()).pathname;
			const j = (body: unknown) =>
				answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
			if (p === '/api/history') return j(guessedHistory);
			if (p === '/api/history/card-lines') {
				return j(cardLinesAnswer(r.request().postDataJSON()));
			}
			if (p === '/api/watched-at') return j({ [guessedHistory[0].id]: 1_700_000_000 });
			// A mapping no play stored: the card's show is a guess.
			if (p.endsWith('/played')) return j(null);
			if (p.startsWith('/api/allmanga-kitsu-map/')) return j(continueKitsuMatch.id);
			if (p === `/api/kitsu/anime/${continueKitsuMatch.id}`) return j(continueKitsuMatch);
			if (p === `/api/kitsu/episodes/${continueKitsuMatch.id}`) return j(continueEpisodes);
			if (p.startsWith('/api/title-match')) return j(null);
			if (p === '/api/history/link-verdict') return j({ offer: 'guess', kept: false });
			if (p === '/api/history/link-group') return j([guessedHistory[0].id]);
			if (p === '/api/history/link-candidates') {
				return j(manyTopRated.slice(0, 6).map((ref) => ({ ref, current: false, warnings: [] })));
			}
			if (p.startsWith('/api/update-check')) {
				return j({
					tag: 'v99.0.0',
					name: 'v99.0.0',
					url: 'https://github.com/JoaoPucci/ani-gui/releases/tag/v99.0.0',
					publishedAt: '2026-01-01T00:00:00Z',
					body: ''
				});
			}
			// Every download request is stubbed: nothing is written there.
			if (p === '/api/download/default-dir') {
				return j(path.join(os.tmpdir(), 'ani-gui-e2e-downloads'));
			}
			return r.fallback();
		});
		await remount(page);
		await silentStreams(page);

		const resume = page.locator('.resume-link').first();
		await expect(resume).toBeVisible({ timeout: 15_000 });
		await expect(page.locator('.update-badge')).toBeVisible({ timeout: 15_000 });
		await expect(page.locator('.toast .action')).toBeVisible({ timeout: 15_000 });
		await page.evaluate(
			() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
		);
		const homeNeeds = [
			'.resume-link',
			'.resume-delete',
			'.update-badge',
			'.toast .action',
			'.toast .dismiss'
		];
		for (const forced of [false, true]) {
			await forcedColours(page, forced);
			for (const key of ['Tab', 'Shift+Tab'] as const) {
				const at = `home with Continue Watching${forced ? ', forced colours' : ''}, ${key}`;
				const stops = await walk(page, key, { selectors: homeNeeds });
				if (forced) assertForced(stops, at, ['brand'], homeNeeds);
				else assertStops(stops, at, ['brand'], homeNeeds);
			}
		}
		await forcedColours(page, false);

		// The link dialog, opened from the card's pill.
		await resume.focus();
		await page.keyboard.press('Enter');
		const dialog = page.getByRole('dialog');
		await expect(dialog).toBeVisible();
		await expect(dialog.locator('.ls-row').first()).toBeVisible({ timeout: 15_000 });
		await page.keyboard.press('Tab');
		await page.keyboard.press('Shift+Tab');
		for (const forced of [false, true]) {
			await forcedColours(page, forced);
			for (const key of ['Tab', 'Shift+Tab'] as const) {
				const at = `the link dialog${forced ? ', forced colours' : ''}, ${key}`;
				const stops = await walk(page, key, { selectors: ['.ls-row'], fresh: false });
				if (forced) assertForced(stops, at, [], ['.ls-row']);
				else assertStops(stops, at, [], ['.ls-row']);
			}
		}
		await forcedColours(page, false);
		await page.keyboard.press('Escape');
		await expect(dialog).toBeHidden();

		// A download in flight shows the bar along the window's bottom.
		await page.locator(`a[href$="/anime/${topRated[0].id}"]`).first().click();
		await expect.poll(() => page.url()).toContain(`/anime/${topRated[0].id}`);
		const download = page.getByRole('button', {
			name: message('detail', 'download_button'),
			exact: true
		});
		await expect(download).toBeEnabled({ timeout: 15_000 });
		await download.click();
		const dlDialog = page.getByRole('dialog');
		await expect(dlDialog).toBeVisible();
		await dlDialog.getByRole('button', { name: message('download', 'button_confirm') }).click();
		await expect(page.locator('.dl-bar-cancel')).toBeVisible({ timeout: 15_000 });
		for (const forced of [false, true]) {
			await forcedColours(page, forced);
			for (const key of ['Tab', 'Shift+Tab'] as const) {
				const at = `a show's page with a download${forced ? ', forced colours' : ''}, ${key}`;
				const stops = await walk(page, key, { selectors: ['.dl-bar-cancel'] });
				if (forced) assertForced(stops, at, [], ['.dl-bar-cancel']);
				else assertStops(stops, at, [], ['.dl-bar-cancel']);
			}
		}
		await forcedColours(page, false);

		// The download list, opened from the topbar: its rows clip what
		// overflows them. It is a menu: the arrow keys move through its
		// actions and wrap, and Tab leaves it (and closes it), so each
		// walk opens it again.
		const openList = async () => {
			await page.locator('.dl-dock-trigger').focus();
			if (!(await page.locator('.dl-dock-pop').isVisible())) await page.keyboard.press('Enter');
			await expect(page.locator('.dl-dock-pop')).toBeVisible();
			await page.locator('.dl-row-act').first().focus();
		};
		for (const forced of [false, true]) {
			await forcedColours(page, forced);
			for (const key of ['ArrowDown', 'ArrowUp'] as const) {
				await openList();
				const at = `the download list${forced ? ', forced colours' : ''}, ${key}`;
				const stops = await walk(page, key, { selectors: ['.dl-row-act'], fresh: false });
				// One download in flight: its one action is the whole list.
				if (forced) assertForced(stops, at, [], ['.dl-row-act'], 1);
				else assertStops(stops, at, [], ['.dl-row-act'], 1);
			}
		}
	} finally {
		await stopApp(app);
	}
});

/** BLEACH's franchise as the backend serves it (the walk's recorded
 *  answer, which the frontend's strip tests read too). */
const bleach = JSON.parse(
	fs.readFileSync(
		path.resolve(__dirname, '../../tests/fixtures/franchise/served-244.json'),
		'utf8'
	)
) as { anchor: string; entries: { id: string; title: string }[] };

/** A show the franchise places mid-run: the third part of BLEACH's
 *  last arc, with an entry before it and one after, while BLEACH itself
 *  is part-watched, so the strip shows its tiles and the line for the
 *  entry being watched. */
const stripShow = {
	...topRated[0],
	id: '48015',
	canonical_title: bleach.entries.find((e) => e.id === '48015')!.title
};

test("the rings on a show page's watch-order strip are drawn once and whole", async () => {
	test.setTimeout(240_000);
	const { app, page } = await launch();
	try {
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await sizeViewport(page, 1440, 900);
		await page.route('**/api/**', async (r) => {
			const p = new URL(r.request().url()).pathname;
			const j = (body: unknown) =>
				answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
			if (p === '/api/kitsu/top-rated') return j([stripShow]);
			if (p.startsWith('/api/kitsu/anime/')) return j(stripShow);
			// The walk answers from the show asked about: the same
			// franchise, anchored there.
			if (p.startsWith('/api/kitsu/franchise/')) {
				return j({ ...bleach, anchor: decodeURIComponent(p.split('/').pop() ?? '') });
			}
			if (p === '/api/history/by-kitsu') {
				const { ids } = r.request().postDataJSON() as { ids: string[] };
				return j(
					Object.fromEntries(
						ids.map((i) => [
							i,
							i === '244' ? { id: 'allmanga-bleach', ep_no: '6', title: 'BLEACH' } : null
						])
					)
				);
			}
			return r.fallback();
		});
		await remount(page);

		await page.locator(`a[href$="/anime/${stripShow.id}"]`).first().click();
		await expect.poll(() => page.url()).toContain(`/anime/${stripShow.id}`);
		await expect(page.locator('.trio .tile').first()).toBeVisible({ timeout: 15_000 });
		const stripNeeds = ['.trio .tile', '.trio .chip'];
		for (const forced of [false, true]) {
			await forcedColours(page, forced);
			for (const key of ['Tab', 'Shift+Tab'] as const) {
				const at = `the watch-order strip${forced ? ', forced colours' : ''}, ${key}`;
				const hovered: Stop[] = [];
				const underMouse = !forced && key === 'Tab';
				const stops = await walk(page, key, {
					selectors: stripNeeds,
					hovered: underMouse ? hovered : undefined
				});
				if (forced) assertForced(stops, at, [], stripNeeds);
				else assertStops(stops, at, [], stripNeeds);
				if (underMouse) assertHovered(hovered, `${at}, under the mouse`);
			}
		}
	} finally {
		await stopApp(app);
	}
});

/** A show the watch-order mockups' fixtures place mid-run: Gintama°. */
const mockShow = { ...topRated[0], id: '10083', canonical_title: 'Gintama°' };

test("the rings on the watch-order mockups' franchise page are drawn once and whole", async () => {
	test.setTimeout(240_000);
	const { app, page } = await launch();
	try {
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await sizeViewport(page, 1440, 900);
		await page.route('**/api/**', async (r) => {
			const p = new URL(r.request().url()).pathname;
			const j = (body: unknown) =>
				answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
			if (p.startsWith('/api/kitsu/anime/')) return j(mockShow);
			return r.fallback();
		});
		// Diagnostics turns the mockups on. No strip links to the page
		// here, so it is opened by its address.
		await page.evaluate(() =>
			localStorage.setItem('ani-gui:watch-order-mockups', JSON.stringify({ enabled: true }))
		);
		await page.goto(new URL(`/anime/${mockShow.id}/franchise`, page.url()).href, {
			waitUntil: 'domcontentloaded'
		});
		await expect(page.locator('.story-row').first()).toBeVisible({ timeout: 15_000 });
		const needs = ['.story-row', '.btn-glass'];
		for (const forced of [false, true]) {
			await forcedColours(page, forced);
			for (const key of ['Tab', 'Shift+Tab'] as const) {
				const at = `the mockups' franchise page${forced ? ', forced colours' : ''}, ${key}`;
				const hovered: Stop[] = [];
				const underMouse = !forced && key === 'Tab';
				const stops = await walk(page, key, {
					selectors: needs,
					hovered: underMouse ? hovered : undefined
				});
				if (forced) assertForced(stops, at, [], needs);
				else assertStops(stops, at, [], needs);
				if (underMouse) assertHovered(hovered, `${at}, under the mouse`);
			}
		}
	} finally {
		await stopApp(app);
	}
});

/** A server for the captions media fixture, standing in for a
 *  provider's CDN (see player-start.spec.ts). */
async function mediaServer(): Promise<{ origin: string; close: () => void }> {
	const dir = path.join(__dirname, 'fixtures', 'captions');
	const types: Record<string, string> = {
		'.m3u8': 'application/vnd.apple.mpegurl',
		'.ts': 'video/mp2t'
	};
	const server = http.createServer((req, res) => {
		const name = path.basename(new URL(req.url ?? '/', 'http://x').pathname);
		const file = path.join(dir, name);
		if (!existsSync(file)) {
			res.writeHead(404).end();
			return;
		}
		const body = fs.readFileSync(file);
		res.writeHead(200, {
			'content-type': types[path.extname(name)] ?? 'application/octet-stream',
			'content-length': body.length
		});
		res.end(body);
	});
	await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
	const { port } = server.address() as net.AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, close: () => server.close() };
}

test("the rings on the player's skip button and its guess note are drawn once and whole", async () => {
	test.setTimeout(180_000);
	const media = await mediaServer();
	try {
		await walkPlayer(media.origin);
	} finally {
		media.close();
	}
});

async function walkPlayer(origin: string) {
	const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-focus-rings-player-'));
	writeReplayConfig(profile, 'locale = "en"\n');
	const app = await launchPackagedApp(
		{
			...process.env,
			XDG_STATE_HOME: path.join(profile, 'state'),
			XDG_CONFIG_HOME: path.join(profile, 'config'),
			XDG_CACHE_HOME: path.join(profile, 'cache'),
			XDG_DATA_HOME: path.join(profile, 'data'),
			ANI_GUI_UNATTENDED: '1'
		},
		{ profile }
	);
	try {
		const page = await appWindow(app);
		await page.emulateMedia({ reducedMotion: 'reduce' });
		const play = await routeRealPlay(page, {
			profile,
			settings: { locale: 'en', use_custom_player_controls: true },
			stream: { upstreamUrl: `${origin}/master.m3u8`, mediaKind: 'hls' }
		});
		// The opening runs from the first second, so the skip button shows;
		// the session is a guess, so the note under the player asks about
		// it.
		await page.route('**/api/**', async (r) => {
			const p = new URL(r.request().url()).pathname;
			const j = (body: unknown) =>
				answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
			if (p.startsWith('/api/aniskip/')) {
				return j([{ skip_type: 'op', start_time: 1, end_time: 600 }]);
			}
			if (p === '/api/history/link-verdict') return j({ offer: 'guess', kept: false });
			if (p === '/api/history/link-group') return j([continueHistory[0].id]);
			if (p === '/api/history/card-lines') {
				return j(cardLinesAnswer(r.request().postDataJSON()));
			}
			// A mapping no play stored: the card's show is a guess.
			if (p.endsWith('/played')) return j(null);
			return r.fallback();
		});
		await remount(page);
		await sizeViewport(page, 1440, 900);
		await playFromContinue(page, play);
		const skip = page.locator('.player-skip-btn');
		await expect(skip).toBeVisible({ timeout: 30_000 });
		const hold = () =>
			page.evaluate(() => {
				const video = document.querySelector('video');
				if (!video) return;
				video.pause();
				video.currentTime = 2;
			});
		await hold();
		await expect(page.locator('.guess-slip .act').first()).toBeVisible({ timeout: 15_000 });
		// A number typed into the episode field turns on its Go button,
		// inside the field's pill.
		await page.locator('input.jump-input').first().fill('3');
		await expect(page.locator('.ep-jump-go').first()).toBeEnabled();
		const needs = ['.player-skip-btn', '.guess-slip .act', '.guess-slip .hide', '.ep-jump-go'];
		for (const forced of [false, true]) {
			await forcedColours(page, forced);
			for (const key of ['Tab', 'Shift+Tab'] as const) {
				// Held inside the button's window while the walk runs.
				await hold();
				await expect(skip).toBeVisible();
				const at = `the player${forced ? ', forced colours' : ''}, ${key}`;
				// Forward, the walk also rests the mouse on every stop.
				const hovered: Stop[] = [];
				const underMouse = !forced && key === 'Tab';
				const stops = await walk(page, key, {
					selectors: needs,
					hovered: underMouse ? hovered : undefined
				});
				if (forced) assertForced(stops, at, [], needs);
				else assertStops(stops, at, [], needs);
				if (underMouse) assertHovered(hovered, `${at}, under the mouse`);
			}
		}
	} finally {
		await stopApp(app);
	}
}

/** The Continue card's show with its sixth episode as its last, and a
 *  franchise in which a second season follows it: the episode the card
 *  plays is the show's finale, so its end brings up the Up next card. */
const finaleShow = { ...continueKitsuMatch, episode_count: 6 };
const finaleEntry = (id: string, title: string, start: string, episodes: number) => ({
	id,
	title,
	subtype: 'TV',
	start,
	episodes,
	status: 'finished',
	poster: null,
	cover: null
});
const finaleFranchise = {
	anchor: finaleShow.id,
	name: finaleShow.canonical_title,
	name_from: finaleShow.id,
	entries: [
		finaleEntry(finaleShow.id, finaleShow.canonical_title, finaleShow.start_date, 6),
		finaleEntry(`${finaleShow.id}-2`, `${finaleShow.canonical_title} Season 2`, '2025-01-10', 12)
	],
	links: [[finaleShow.id, `${finaleShow.id}-2`]],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
};

test("the rings on the player's Up next card are drawn once and whole", async () => {
	test.setTimeout(180_000);
	const media = await mediaServer();
	const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-focus-rings-up-next-'));
	writeReplayConfig(profile, 'locale = "en"\n');
	const app = await launchPackagedApp(
		{
			...process.env,
			XDG_STATE_HOME: path.join(profile, 'state'),
			XDG_CONFIG_HOME: path.join(profile, 'config'),
			XDG_CACHE_HOME: path.join(profile, 'cache'),
			XDG_DATA_HOME: path.join(profile, 'data'),
			ANI_GUI_UNATTENDED: '1'
		},
		{ profile }
	);
	try {
		const page = await appWindow(app);
		await page.emulateMedia({ reducedMotion: 'reduce' });
		const play = await routeRealPlay(page, {
			profile,
			settings: { locale: 'en', use_custom_player_controls: true },
			stream: { upstreamUrl: `${media.origin}/master.m3u8`, mediaKind: 'hls' },
			show: { match: finaleShow, episode: continueKitsuEpisode6 }
		});
		await page.route('**/api/**', async (r) => {
			const p = new URL(r.request().url()).pathname;
			const j = (body: unknown) =>
				answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
			if (p.startsWith('/api/kitsu/franchise/')) return j(finaleFranchise);
			if (p === '/api/history/by-kitsu') {
				const { ids } = r.request().postDataJSON() as { ids: string[] };
				return j(Object.fromEntries(ids.map((i) => [i, null])));
			}
			return r.fallback();
		});
		await remount(page);
		await sizeViewport(page, 1440, 900);
		await playFromContinue(page, play);
		// The end of the finale brings the card up.
		await expect(page.locator('video')).toBeVisible({ timeout: 30_000 });
		await expect
			.poll(() => page.evaluate(() => document.querySelector('video')?.duration ?? 0), {
				timeout: 30_000
			})
			.toBeGreaterThan(0);
		await page.evaluate(() => {
			const video = document.querySelector('video')!;
			video.currentTime = video.duration;
		});
		await expect(page.locator('.up-next a.row').first()).toBeVisible({ timeout: 30_000 });
		const needs = ['.up-next a.row', '.up-next .dismiss'];
		for (const forced of [false, true]) {
			await forcedColours(page, forced);
			for (const key of ['Tab', 'Shift+Tab'] as const) {
				const at = `the Up next card${forced ? ', forced colours' : ''}, ${key}`;
				const hovered: Stop[] = [];
				const underMouse = !forced && key === 'Tab';
				const stops = await walk(page, key, {
					selectors: needs,
					hovered: underMouse ? hovered : undefined
				});
				if (forced) assertForced(stops, at, [], needs);
				else assertStops(stops, at, [], needs);
				if (underMouse) assertHovered(hovered, `${at}, under the mouse`);
			}
		}
	} finally {
		await stopApp(app);
		media.close();
	}
});

test('in a forced-colours theme every focused control draws an outline', async () => {
	test.setTimeout(240_000);
	const { app, page } = await launch();
	try {
		// A contrast theme (Windows) drops every box shadow, so a ring
		// drawn only as a shadow is not there; an outline is kept.
		await forcedColours(page, true);
		await sizeViewport(page, 1440, 900);
		for (const p of pagesOf(page)) {
			await page.locator('a.brand').click();
			await p.go();
			await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
			for (const key of ['Tab', 'Shift+Tab'] as const) {
				const at = `${p.label}, forced colours, ${key}`;
				assertForced(await walk(page, key, { selectors: p.need }), at, p.required, p.need);
			}
		}
	} finally {
		await stopApp(app);
	}
});

test("the hero's empty and failed states ring their button, under the mouse too", async () => {
	test.setTimeout(180_000);
	const { page, app } = await launch();
	try {
		await sizeViewport(page, 1440, 900);
		type Route = Parameters<Parameters<Page['route']>[1]>[0];
		const states = [
			// Trending answered with no show to feature.
			{
				label: 'empty',
				answer: (r: Route) =>
					answer(r, { status: 200, contentType: 'application/json', body: '[]' })
			},
			// Trending did not answer.
			{ label: 'failed', answer: (r: Route) => answer(r, { status: 503 }) }
		];
		for (const state of states) {
			// The latest route answers first; every other request falls
			// back to the launch's stubs.
			await page.route('**/api/**', async (r) => {
				const p = new URL(r.request().url()).pathname;
				if (p === '/api/kitsu/trending-anilist') return state.answer(r);
				return r.fallback();
			});
			await remount(page);
			const cta = page.locator('.hero-empty .btn-primary');
			await expect(cta).toBeVisible({ timeout: 15_000 });
			for (const forced of [false, true]) {
				await forcedColours(page, forced);
				const at = `the hero, ${state.label}${forced ? ', forced colours' : ''}`;
				const hovered: Stop[] = [];
				const stops = await walk(page, 'Tab', {
					selectors: ['.btn-primary'],
					hovered: forced ? undefined : hovered
				});
				if (forced) assertForced(stops, at, ['brand'], ['.btn-primary']);
				else {
					assertStops(stops, at, ['brand'], ['.btn-primary']);
					assertHovered(hovered, `${at}, under the mouse`);
				}
			}
			await forcedColours(page, false);
		}
	} finally {
		await stopApp(app);
	}
});
