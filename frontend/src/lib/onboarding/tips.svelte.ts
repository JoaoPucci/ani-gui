/**
 * The feature tips offered to this install, as the backend answered
 * them once for the session (`GET /api/onboarding`), and the closes
 * the renderer has sent since. The backend decides which tips are
 * offered and pending; this only keeps its answer and mirrors each
 * close, so a tip closed is not shown again before the next launch
 * reads the record.
 */
import {
	onboardingSeen,
	onboardingTips,
	type OnboardingHowTo,
	type OnboardingTip,
	type OnboardingView
} from '$lib/api';

export class OnboardingTips {
	/** The answer, or null until it arrives (or when it never did). */
	view = $state<OnboardingView | null>(null);
	/** A tip asked for on demand (the Debug page's Show it): it shows
	 *  the next time its first place settles on its page, whether or not
	 *  it is pending, and closing it marks nothing seen. */
	armed = $state<OnboardingTip | null>(null);
	/** How the armed tip shows: a Debug preview, which looks as it would
	 *  when offered, or a walkthrough a feature's "Show me how" asked for,
	 *  which says "How it works" and names no version. */
	armedAs = $state<'preview' | 'guide'>('preview');
	#asked: Promise<void> | null = null;

	readonly #fetchTips: () => Promise<OnboardingView>;
	readonly #sendSeen: (id: string) => Promise<void>;

	constructor(
		fetchTips: () => Promise<OnboardingView> = () => onboardingTips(),
		sendSeen: (id: string) => Promise<void> = onboardingSeen
	) {
		this.#fetchTips = fetchTips;
		this.#sendSeen = sendSeen;
	}

	/** The tips offered, seen or not, in order. */
	get tips(): OnboardingTip[] {
		return this.view?.tips ?? [];
	}

	/** Ask the backend once. A failed read leaves no tips for the
	 *  session: a tip not shown is the safe side. */
	load(): Promise<void> {
		this.#asked ??= this.#fetchTips().then(
			(view) => {
				this.view = view;
			},
			() => {}
		);
		return this.#asked;
	}

	/** A seen state set elsewhere (the Debug page's toggle): the session
	 *  takes it, sending nothing. */
	seenHere(id: string, seen: boolean): void {
		if (!this.view) return;
		this.view = {
			...this.view,
			tips: this.view.tips.map((t) => (t.id === id ? { ...t, pending: !seen } : t))
		};
	}

	/** Show `tip` the next time its first place settles on screen. */
	showNow(tip: OnboardingTip): void {
		this.armed = tip;
		this.armedAs = 'preview';
	}

	/** Run a feature's walkthrough the next time its first place settles
	 *  on its page. */
	showGuide(howTo: OnboardingHowTo): void {
		this.armed = {
			id: howTo.id,
			kind: 'sequence',
			surface: howTo.surface,
			moment: 'arrive',
			since: '',
			places: howTo.places,
			how_to: null,
			pending: true
		};
		this.armedAs = 'guide';
	}

	/** A page came up on `surface`: a walkthrough asked for that runs
	 *  elsewhere is dropped, so it never shows unasked on a later visit.
	 *  A Debug preview waits until its page comes. */
	pageChanged(surface: string): void {
		if (this.armedAs === 'guide' && this.armed && this.armed.surface !== surface) this.armed = null;
	}

	/** A tip asked for gave way before it was closed: it is asked for
	 *  again, as it was. */
	rearm(tip: OnboardingTip, as: 'preview' | 'guide'): void {
		this.armed = tip;
		this.armedAs = as;
	}

	/** The tip asked for has shown. */
	disarm(): void {
		this.armed = null;
	}

	/** Ask again on the next load (tests). */
	forget(): void {
		this.#asked = null;
	}

	/** A tip was closed: never pending again. Sent to the backend; a
	 *  close it did not take still holds for this session. */
	markSeen(id: string): void {
		if (this.view) {
			this.view = {
				...this.view,
				tips: this.view.tips.map((t) => (t.id === id ? { ...t, pending: false } : t))
			};
		}
		void this.#sendSeen(id).catch(() => {});
	}
}

/** The session's tips. */
export const onboarding = new OnboardingTips();

/** Tests only: the session's tips as before the first read. */
export function __resetOnboardingForTests(): void {
	onboarding.view = null;
	onboarding.armed = null;
	onboarding.forget();
}
