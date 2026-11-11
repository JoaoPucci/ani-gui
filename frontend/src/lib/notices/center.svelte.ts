/**
 * The notification center's state: the list, whether its panel is
 * open, and the toasts held while something is fullscreen. One
 * instance lives for the window (`noticeCenter`, ./instance). Creating
 * it reads and writes nothing; the layout loads it only while the
 * center's flag is on, so with the flag off its storage is never
 * touched.
 *
 * The rules live in pure modules — the list in ./model, arrival in
 * ./arrival — and this class only holds their state and calls out:
 * to storage for events, and to `toast` for a notice that arrives as
 * a toast.
 */
import { IDLE, arrival, release, type Moment } from './arrival';
import {
	STORAGE_KEY,
	bellTone,
	dismiss,
	markAllRead,
	markRead,
	prune,
	raise,
	resolveKey,
	restore,
	serialize,
	unreadCount,
	withdraw,
	type Change,
	type Notice,
	type NoticeData,
	type RaiseInput
} from './model';
import type { SourceChange } from './sources';

/** The storage the center keeps its events in. */
export type NoticeStorage = Pick<Storage, 'getItem' | 'setItem'>;

export interface CenterDeps {
	storage: NoticeStorage | null;
	now: () => number;
	/** Shows a notice as a toast. */
	toast: (n: Notice) => void;
	/** What the window is doing right now, read at each arrival. The
	 *  last moment reported to `setMoment` stands in without it. */
	moment?: () => Moment;
	/** Told the rows before and after each change to the list, for what
	 *  is remembered outside it (the published notices read or
	 *  dismissed). */
	observe?: (before: readonly Notice[], after: readonly Notice[]) => void;
}

export class NoticeCenter {
	rows = $state<Notice[]>([]);
	open = $state(false);
	/** The row opened in place in the panel (on the player, where
	 *  nothing navigates away). */
	expanded = $state<string | null>(null);
	moment = $state<Moment>(IDLE);
	private held: string[] = [];

	constructor(private readonly deps: CenterDeps) {}

	/** Read the stored events. Whatever storage refuses is treated as
	 *  nothing stored. */
	load(): void {
		let raw: string | null;
		try {
			raw = this.deps.storage?.getItem(STORAGE_KEY) ?? null;
		} catch {
			raw = null;
		}
		this.rows = restore(raw, this.deps.now());
	}

	count(updateOnBadge: boolean): number {
		return unreadCount(this.rows, { updateOnBadge });
	}

	tone(updateOnBadge: boolean): 'attention' | 'neutral' {
		return bellTone(this.rows, { updateOnBadge });
	}

	raise(input: RaiseInput): void {
		this.apply(raise(this.rows, input, this.deps.now()));
	}

	resolve(key: string, at: number, data?: NoticeData): void {
		this.apply(resolveKey(this.rows, key, at, data));
	}

	/** Apply what a source asks for. */
	take(changes: readonly SourceChange[]): void {
		for (const c of changes) {
			if (c.op === 'raise') this.raise(c.input);
			else if (c.op === 'resolve') this.resolve(c.key, c.at ?? this.deps.now(), c.data);
			else this.withdraw(c.key);
		}
	}

	private withdraw(key: string): void {
		const next = withdraw(this.rows, key);
		if (next.length !== this.rows.length) this.set(next);
	}

	markRead(key: string): void {
		this.set(markRead(this.rows, key));
	}

	markAllRead(): void {
		this.set(markAllRead(this.rows));
	}

	dismiss(key: string): void {
		if (this.expanded === key) this.expanded = null;
		this.set(dismiss(this.rows, key));
	}

	/** What the window is doing changed: held toasts show once it
	 *  leaves fullscreen. */
	setMoment(at: Moment): void {
		this.moment = at;
		const out = release(this.held, this.rows, at);
		this.held = out.held;
		for (const n of out.show) this.deps.toast(n);
	}

	private apply(c: Change): void {
		// A change always touches a row; one that changes nothing has none.
		if (!c.changed || !c.notice) return;
		this.set(c.list);
		const how = arrival(c.notice, c.fresh, this.deps.moment?.() ?? this.moment);
		if (how === 'toast') this.deps.toast(c.notice);
		else if (how === 'hold') this.held.push(c.notice.key);
	}

	private set(list: Notice[]): void {
		const before = this.rows;
		this.rows = prune(list, this.deps.now());
		this.deps.observe?.(before, this.rows);
		try {
			this.deps.storage?.setItem(STORAGE_KEY, serialize(this.rows));
		} catch {
			// Storage full or refused: the list still works this session.
		}
	}
}
