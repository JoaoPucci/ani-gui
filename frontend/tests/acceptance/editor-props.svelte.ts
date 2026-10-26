// Reactive props for a mounted list editor, so a scenario can move it to
// another show the way the detail route does: one component, a new
// `kitsuId`.

import type { EntryView } from '../../src/lib/account/types';

export const editorProps = $state<{
	kitsuId: string;
	total: number | null;
	current: EntryView | null;
}>({ kitsuId: '42', total: 12, current: { status: 'watching', progress: 3 } });

/** Point the editor at `kitsuId`, as the detail route does on navigation. */
export function showEditorFor(kitsuId: string): void {
	editorProps.kitsuId = kitsuId;
}
