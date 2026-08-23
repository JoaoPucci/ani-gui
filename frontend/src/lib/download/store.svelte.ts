/**
 * Shared download state — a Svelte 5 rune store that lives outside any
 * single component so the topbar dock, the bottom progress strip, and
 * the per-download confirm modal all observe the same list.
 *
 * Lifecycle of a download item:
 *   1. `addPending(args)` returns the new id; status = "pending"
 *      (modal opened, awaiting user confirm).
 *   2. `markActive(id)` flips to "active" and starts the SSE stream.
 *      `setProgress(id, line)` updates the latest progress line.
 *   3. `markDone(id, destDir)` flips to "done"; renderer can then
 *      offer "reveal in folder".
 *   4. `markError(id, message)` flips to "error".
 *   5. `dismiss(id)` removes the row.
 *
 * Active downloads also carry an `AbortController` so the topbar
 * dock's cancel button can abort the in-flight fetch — the SSE
 * connection closes, the backend's `kill_on_drop(true)` reaps the
 * downloader child.
 */

export type DownloadStatus = 'pending' | 'active' | 'done' | 'error';

export interface DownloadItem {
	id: string;
	title: string;
	/** Episode arg as sent to the backend — `"5"` for single, `"5-12"` for range. */
	episode: string;
	mode: string;
	quality: string;
	destDir: string;
	status: DownloadStatus;
	progress: string | null;
	error: string | null;
	startedAt: number;
	abort: AbortController | null;
	/** True when status flipped to "done" or "error" while the user
	 *  wasn't looking at the dock. Cleared the next time the dock
	 *  opens — drives the small completion badge on the topbar icon. */
	unseen: boolean;
	/** When the episode arg is `"M-N"`, the count of episodes in the
	 *  range — drives the dock's "Episode N of M" annotation. Null
	 *  for single-episode downloads. */
	rangeTotal: number | null;
	/** Last episode number seen in a `Playing episode N` line. The
	 *  backend's range loop emits those itself, before it resolves or
	 *  spawns anything — no tool prints them. Updated by setProgress as
	 *  lines arrive; null until the first one is parsed. */
	currentEp: number | null;
	/** Parsed from the latest progress line when it is one of the
	 *  backend's own `status.download.*` reports rather than tool
	 *  output. The backend sends stable keys — the sentence lives in
	 *  the message bundles — and the dock renders the translation.
	 *  Cleared when a later raw line supersedes the report. */
	progressStatus: ProgressStatus | null;
}

/** One of the backend's own progress reports, as a stable key the UI
 *  translates. `path`, where present, follows the key after the first
 *  space and is kept verbatim — it names the user's file, whatever
 *  characters the title brought with it. */
export interface ProgressStatus {
	key: ProgressStatusKey;
	path: string | null;
}

export type ProgressStatusKey =
	| 'already_here'
	| 'abandoned_claim'
	| 'claim_pending'
	| 'repackage_retry'
	| 'retry_ffmpeg';

const PROGRESS_STATUS_KEYS: readonly ProgressStatusKey[] = [
	'already_here',
	'abandoned_claim',
	'claim_pending',
	'repackage_retry',
	'retry_ffmpeg'
];

/** The reports that explain an ended download — the last thing the
 *  backend says before `done` or `error`, and therefore the ones a
 *  terminal row renders. The retries are deliberately not here: a
 *  download that retried through ffmpeg and later failed for another
 *  reason did not fail because of the retry, and a finished row
 *  claiming "retrying" would be false. */
export function terminalReport(status: ProgressStatus | null): ProgressStatus | null {
	if (!status) return null;
	switch (status.key) {
		case 'already_here':
		case 'abandoned_claim':
		case 'claim_pending':
			return status;
		case 'repackage_retry':
		case 'retry_ffmpeg':
			return null;
	}
}

/** Recognize a backend progress report. Returns null for tool output
 *  and for `status.download.*` names this build does not know — an
 *  older frontend against a newer backend then shows the raw line,
 *  which is still true, rather than nothing. */
export function parseProgressStatus(line: string): ProgressStatus | null {
	const prefix = 'status.download.';
	if (!line.startsWith(prefix)) return null;
	const rest = line.slice(prefix.length);
	const space = rest.indexOf(' ');
	const name = space === -1 ? rest : rest.slice(0, space);
	const path = space === -1 ? null : rest.slice(space + 1);
	const key = PROGRESS_STATUS_KEYS.find((k) => k === name);
	if (!key) return null;
	return { key, path: path && path.length > 0 ? path : null };
}

let nextId = 1;

class DownloadStore {
	items = $state<DownloadItem[]>([]);

	get active(): DownloadItem[] {
		return this.items.filter((i) => i.status === 'pending' || i.status === 'active');
	}
	get hasActive(): boolean {
		return this.active.length > 0;
	}
	/** Items the dock hasn't surfaced yet — drives the small completion
	 *  badge on the topbar download icon. Cleared by `markAllSeen()`. */
	get unseenCount(): number {
		return this.items.reduce((n, i) => n + (i.unseen ? 1 : 0), 0);
	}

	add(args: {
		title: string;
		episode: string;
		mode: string;
		quality: string;
		destDir: string;
	}): string {
		const id = `dl-${nextId++}`;
		// Parse `"M-N"` to compute the range size up front so the dock
		// can show "Episode K of N-M+1" before any progress arrives.
		const rangeMatch = args.episode.match(/^(\d+)-(\d+)$/);
		const rangeTotal = rangeMatch
			? Math.max(1, Number.parseInt(rangeMatch[2], 10) - Number.parseInt(rangeMatch[1], 10) + 1)
			: null;
		this.items = [
			{
				id,
				title: args.title,
				episode: args.episode,
				mode: args.mode,
				quality: args.quality,
				destDir: args.destDir,
				status: 'pending',
				progress: null,
				error: null,
				startedAt: Date.now(),
				abort: null,
				unseen: false,
				rangeTotal,
				currentEp: null,
				progressStatus: null
			},
			...this.items
		];
		return id;
	}

	markActive(id: string, abort: AbortController) {
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, status: 'active', abort, startedAt: Date.now() } : i
		);
	}

	setProgress(id: string, line: string) {
		// A range download emits `Playing episode N` before it resolves each
		// episode. That line comes from the backend's own range loop, not
		// from yt-dlp or ffmpeg — it is a protocol line the orchestrator
		// mixes into the tool's stderr, so changing its shape means
		// changing `download_range.rs` and this parse together. Parsed so
		// the dock can show "Episode N of M" instead of the raw line.
		const match = line.match(/^Playing episode\s+(\d+(?:\.\d+)?)/i);
		const currentEp = match ? Number.parseFloat(match[1]) : null;
		const progressStatus = parseProgressStatus(line);
		this.items = this.items.map((i) =>
			i.id === id
				? { ...i, progress: line, currentEp: currentEp ?? i.currentEp, progressStatus }
				: i
		);
	}

	markDone(id: string, destDir: string) {
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, status: 'done', destDir, abort: null, unseen: true } : i
		);
	}

	markError(id: string, message: string) {
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, status: 'error', error: message, abort: null, unseen: true } : i
		);
	}

	/** Called when the dock opens — clears the unseen flag on every
	 *  done/errored item so the topbar dot fades. */
	markAllSeen() {
		if (this.items.every((i) => !i.unseen)) return;
		this.items = this.items.map((i) => (i.unseen ? { ...i, unseen: false } : i));
	}

	cancel(id: string) {
		const item = this.items.find((i) => i.id === id);
		if (!item) return;
		if (item.abort) item.abort.abort();
		// User-initiated cancel — drop the row immediately. The catch
		// handler in start.ts will fire after the abort signal
		// propagates, but markError's .map finds no item by id at
		// that point and the call is a no-op. (Prevents the row
		// briefly flashing the error/red state on cancel.)
		this.dismiss(id);
	}

	dismiss(id: string) {
		this.items = this.items.filter((i) => i.id !== id);
	}
}

export const downloadStore = new DownloadStore();
