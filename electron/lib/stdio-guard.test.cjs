'use strict';

// A write to the main process's stdout or stderr that fails — the
// terminal the app was started from has closed (EIO), the reader of a
// pipe has gone (EPIPE), the stream was ended — never throws into its
// caller and never reaches the process as an uncaught exception, and
// the stream takes no further writes once one has failed.

const test = require('node:test');
const assert = require('node:assert/strict');
const { Writable } = require('node:stream');
const { spawn } = require('node:child_process');

const { guardStdio, guardStream } = require('./stdio-guard.cjs');

/** A stream whose every write fails with `code`, reported the way a
 *  socket, pipe or terminal reports it: through the write's callback
 *  and then as an 'error' event. Like Node's own stdout and stderr, it
 *  is never left destroyed: their destroy puts the stream back, so
 *  each later write tries the dead terminal again and fails again. */
function failingStream(code) {
	const writes = [];
	const stream = new Writable({
		write(chunk, _encoding, callback) {
			writes.push(String(chunk));
			callback(Object.assign(new Error(`write ${code}`), { code }));
		},
		destroy(err, callback) {
			callback(err);
			this._undestroy();
		}
	});
	return { stream, writes };
}

/** Lets every pending tick and immediate run: an 'error' event a failed
 *  write emits lands within them. */
const settle = () => new Promise((resolve) => setImmediate(() => setImmediate(resolve)));

/** Fails the test on an uncaught exception while `body` runs and the
 *  events it caused settle. */
async function withoutUncaught(body) {
	const seen = [];
	const record = (err) => seen.push(err);
	process.on('uncaughtException', record);
	try {
		await body();
		await settle();
	} finally {
		process.removeListener('uncaughtException', record);
	}
	assert.deepEqual(seen, []);
}

for (const code of ['EIO', 'EPIPE', 'ECONNRESET']) {
	test(`a write failing with ${code} reaches no one as an uncaught exception`, async () => {
		const { stream, writes } = failingStream(code);
		guardStream(stream);
		await withoutUncaught(async () => {
			stream.write('first\n');
		});
		assert.deepEqual(writes, ['first\n']);
	});
}

test('once a write has failed, the stream takes no further writes', async () => {
	const { stream, writes } = failingStream('EIO');
	guardStream(stream);
	await withoutUncaught(async () => {
		stream.write('first\n');
		await settle();
		stream.write('second\n');
		stream.write('third\n');
	});
	assert.deepEqual(writes, ['first\n']);
});

test('a write after the failure still calls its callback, so nothing waits on it', async () => {
	const { stream } = failingStream('EPIPE');
	guardStream(stream);
	await withoutUncaught(async () => {
		stream.write('first\n');
		await settle();
		await new Promise((resolve) => stream.write('second\n', resolve));
		await new Promise((resolve) => stream.write('third\n', 'utf8', resolve));
	});
});

test('a write that throws, as a file stream does, does not throw into its caller', async () => {
	let calls = 0;
	const stream = new Writable({ write() {} });
	stream.write = () => {
		calls += 1;
		throw Object.assign(new Error('write EIO'), { code: 'EIO' });
	};
	guardStream(stream);
	await withoutUncaught(async () => {
		assert.doesNotThrow(() => stream.write('first\n'));
		assert.doesNotThrow(() => stream.write('second\n'));
	});
	assert.equal(calls, 1);
});

test('a write after the stream was ended reaches no one as an uncaught exception', async () => {
	const writes = [];
	const stream = new Writable({
		write(chunk, _encoding, callback) {
			writes.push(String(chunk));
			callback();
		}
	});
	guardStream(stream);
	await withoutUncaught(async () => {
		stream.end('last\n');
		await settle();
		stream.write('after the end\n');
	});
	assert.deepEqual(writes, ['last\n']);
});

test('a healthy stream gets every write as it was made', async () => {
	const writes = [];
	const stream = new Writable({
		decodeStrings: false,
		write(chunk, encoding, callback) {
			writes.push([chunk, encoding]);
			callback();
		}
	});
	guardStream(stream);
	let called = false;
	const accepted = stream.write('one\n', 'latin1', () => {
		called = true;
	});
	stream.write('two\n');
	await settle();
	assert.equal(accepted, true);
	assert.equal(called, true);
	assert.deepEqual(writes, [
		['one\n', 'latin1'],
		['two\n', 'utf8']
	]);
});

/** Runs `script` in a child Node whose stdout and stderr are pipes the
 *  test closes at once, so every write the child makes finds no reader;
 *  resolves with how the child ended and what it reported on fd 3. */
function runWithReaderGone(script) {
	return new Promise((resolve, reject) => {
		const child = spawn(process.execPath, ['-e', script], {
			stdio: ['ignore', 'pipe', 'pipe', 'pipe']
		});
		child.stdout.destroy();
		child.stderr.destroy();
		let report = '';
		child.stdio[3].on('data', (d) => {
			report += d;
		});
		child.once('error', reject);
		child.once('close', (code, signal) => resolve({ code, signal, report }));
	});
}

// What every writer in the main process does — console.*, the backend
// log relay's process.stdout/stderr.write — run against Node's own
// stdio streams, with the pipes' reader gone.
const WRITERS = `
const fs = require('node:fs');
const write = () => {
	console.log('[main] a line');
	console.error('[main] an error');
	process.stdout.write('[backend] a line\\n');
	process.stderr.write('[backend] a warning\\n');
};
let rounds = 0;
const tick = setInterval(() => {
	write();
	rounds += 1;
	if (rounds === 20) {
		clearInterval(tick);
		fs.writeSync(3, 'survived');
	}
}, 5);
`;

test('the main process outlives every write to stdout and stderr once their reader has gone', async () => {
	const guard = `require(${JSON.stringify(require.resolve('./stdio-guard.cjs'))}).guardStdio(process);`;
	const run = await runWithReaderGone(guard + WRITERS);
	assert.deepEqual(run, { code: 0, signal: null, report: 'survived' });
});

test('the same writes take an unguarded process down, the failure guardStdio is there for', async () => {
	const run = await runWithReaderGone(WRITERS);
	assert.notEqual(run.code, 0);
	assert.notEqual(run.report, 'survived');
});

test('guardStdio guards the process stdout and stderr', () => {
	const guarded = [];
	const proc = { stdout: { name: 'stdout' }, stderr: { name: 'stderr' } };
	guardStdio(proc, { guard: (stream) => guarded.push(stream.name) });
	assert.deepEqual(guarded, ['stdout', 'stderr']);
});
