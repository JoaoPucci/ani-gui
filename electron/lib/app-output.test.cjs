// The record of what a launched app said, which a failing e2e test
// carries in its report (e2e/launch.ts, e2e/policy-guard.ts).

const test = require('node:test');
const assert = require('node:assert/strict');
const fc = require('fast-check');

const { MAX_LINES, MAX_LINE_CHARS, outputRecorder } = require('./app-output.cjs');

test('each line is kept with the stream it came on, in the order it came', () => {
	const r = outputRecorder('app pid 7');
	r.feed('stdout', '[main] splash asked for\n');
	r.feed('stderr', 'Debugger listening\n');
	r.feed('stdout', Buffer.from('[main] splash shown\n'));
	r.exited(0, null);
	assert.equal(
		r.text(),
		['app pid 7: exited with code 0', '[stdout] [main] splash asked for', '[stderr] Debugger listening', '[stdout] [main] splash shown'].join('\n')
	);
});

test('a line split across chunks is one line, and a character split across Buffers is whole', () => {
	const r = outputRecorder('app');
	const bytes = Buffer.from('[main] ré\n');
	const cut = bytes.indexOf(0xc3) + 1;
	r.feed('stdout', bytes.subarray(0, cut));
	r.feed('stdout', bytes.subarray(cut));
	r.feed('stderr', 'half');
	r.feed('stderr', ' and half\n');
	assert.deepEqual(r.text().split('\n').slice(1), ['[stdout] [main] ré', '[stderr] half and half']);
});

test('how the app ended: a code, a signal, a spawn that failed, or not yet', () => {
	const running = outputRecorder('app');
	assert.equal(running.text(), 'app: still running when the test ended');
	const failed = outputRecorder('app');
	failed.exited(1, null);
	assert.equal(failed.text(), 'app: exited with code 1');
	const killed = outputRecorder('app');
	killed.exited(null, 'SIGTRAP');
	assert.equal(killed.text(), 'app: ended by SIGTRAP');
	const never = outputRecorder('app');
	never.failed(new Error('spawn /nowhere ENOENT'));
	assert.equal(never.text(), 'app: did not start (spawn /nowhere ENOENT)');
});

test('a line longer than the cap is cut, and says by how much, whether it came whole or in pieces', () => {
	const r = outputRecorder('app', { maxLineChars: 5 });
	r.feed('stdout', 'abcdefgh\n');
	r.feed('stderr', 'abc');
	r.feed('stderr', 'defg');
	assert.deepEqual(r.text().split('\n').slice(1), [
		'[stdout] abcde … (3 more characters not kept)',
		'[stderr] abcde … (2 more characters not kept)'
	]);
	r.feed('stderr', 'h\nok\n');
	assert.deepEqual(r.text().split('\n').slice(1), [
		'[stdout] abcde … (3 more characters not kept)',
		'[stderr] abcde … (3 more characters not kept)',
		'[stderr] ok'
	]);
	assert.equal(MAX_LINE_CHARS, 4000);
});

test('the last lines alone, for a report that has room for few, say how many came before', () => {
	const r = outputRecorder('app', { maxLines: 4 });
	for (let i = 1; i <= 6; i++) r.feed('stdout', `line ${i}\n`);
	r.feed('stderr', 'waiting');
	assert.deepEqual(r.text({ last: 2 }).split('\n'), [
		'app: still running when the test ended',
		'(5 earlier lines not shown)',
		'[stdout] line 6',
		'[stderr] waiting'
	]);
});

test('a last line with no newline is kept when its stream ends, and shown while it waits', () => {
	const r = outputRecorder('app');
	r.feed('stdout', 'no newline yet');
	assert.deepEqual(r.text().split('\n').slice(1), ['[stdout] no newline yet']);
	r.end('stdout');
	r.end('stderr');
	assert.deepEqual(r.text().split('\n').slice(1), ['[stdout] no newline yet']);
});

test('only the last lines are kept, and the record says how many went', () => {
	const r = outputRecorder('app', { maxLines: 3 });
	for (let i = 1; i <= 5; i++) r.feed('stdout', `line ${i}\n`);
	assert.deepEqual(r.text().split('\n').slice(1), ['(2 earlier lines not shown)', '[stdout] line 3', '[stdout] line 4', '[stdout] line 5']);
	assert.equal(MAX_LINES, 1000);
});

test('however the output is cut into chunks, the record holds the same lines', () => {
	fc.assert(
		fc.property(
			fc.array(fc.stringMatching(/^[a-zé [\]]{0,12}$/), { maxLength: 30 }),
			fc.array(fc.nat(), { maxLength: 20 }),
			fc.integer({ min: 1, max: 40 }),
			fc.integer({ min: 1, max: 15 }),
			(lines, cuts, maxLines, maxLineChars) => {
				const whole = Buffer.from(lines.map((l) => `${l}\n`).join(''));
				const points = [...new Set(cuts.map((c) => (whole.length ? c % whole.length : 0)))].sort((a, b) => a - b);
				const r = outputRecorder('app', { maxLines, maxLineChars });
				let at = 0;
				for (const p of [...points, whole.length]) {
					r.feed('stdout', whole.subarray(at, p));
					at = p;
				}
				r.end('stdout');
				const kept = lines.slice(-maxLines).map((l) =>
					l.length > maxLineChars
						? `[stdout] ${l.slice(0, maxLineChars)} … (${l.length - maxLineChars} more characters not kept)`
						: `[stdout] ${l}`
				);
				const cut = lines.length > maxLines ? [`(${lines.length - maxLines} earlier lines not shown)`] : [];
				assert.deepEqual(r.text().split('\n').slice(1), [...cut, ...kept]);
			}
		)
	);
});
