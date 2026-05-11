/**
 * Stub for the drag-generation guard. Real implementation lands in
 * the green commit; this stub exists so the red commit's test file
 * can import without breaking compilation.
 */

export function isStaleDragCallback(scheduledGen: number, currentGen: number): boolean {
	void scheduledGen;
	void currentGen;
	return false;
}
