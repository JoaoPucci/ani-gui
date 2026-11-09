/**
 * The error notice after an account action whose bridge call rejected.
 * The app cannot tell whether the action landed, so the disk is read
 * again (`reread`) and the notice shows only if the read does not show
 * it landed. A keyring that takes its time does not hold the notice
 * back: past `waitMs` it shows, and it is withdrawn if the read then
 * shows the action landed after all.
 */

export interface LandedNoticeOptions {
	/** Read the provider again; true when the answer was applied. */
	reread(): Promise<boolean>;
	/** Whether the provider, as read, shows the action landed. */
	landed(): boolean;
	/** Show the notice; returns its id. */
	push(): string;
	dismiss(id: string): void;
	waitMs: number;
	setTimer(fn: () => void, ms: number): unknown;
	clearTimer(handle: unknown): void;
}

export async function noticeUnlessLanded(o: LandedNoticeOptions): Promise<void> {
	let shown: string | null = null;
	const timer = o.setTimer(() => {
		shown = o.push();
	}, o.waitMs);
	const applied = await o.reread();
	o.clearTimer(timer);
	const landed = applied && o.landed();
	if (shown !== null) {
		if (landed) o.dismiss(shown);
		return;
	}
	if (!landed) o.push();
}

/** The real timers, for callers outside tests. */
export const realTimers = {
	setTimer: (fn: () => void, ms: number): unknown => setTimeout(() => fn(), ms),
	clearTimer: (h: unknown): void => clearTimeout(h as ReturnType<typeof setTimeout>)
};
