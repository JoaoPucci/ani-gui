/**
 * The feature tips offered to this install, as the backend answered
 * them once for the session (`GET /api/onboarding`), and the closes
 * the renderer has sent since. The backend decides which tips are
 * offered and pending; this only keeps its answer and mirrors each
 * close, so a tip closed is not shown again before the next launch
 * reads the record.
 */
import { onboardingSeen, onboardingTips, type OnboardingTip, type OnboardingView } from '$lib/api';

export class OnboardingTips {
	/** The answer, or null until it arrives (or when it never did). */
	view = $state<OnboardingView | null>(null);
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
	onboarding.forget();
}
