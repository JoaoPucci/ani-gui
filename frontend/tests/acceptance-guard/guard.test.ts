// The acceptance tier's request guard (tests/acceptance/setup.ts),
// driven through its cases.
//
// A guard that fails scenarios cannot be exercised by a scenario that
// passes, so each case is a fixture under fixtures/: a file of
// scenarios run by a child vitest under the acceptance tier's own
// config (vitest.fixtures.config.ts), whose JSON report this file
// reads. The assertion is on which scenario failed, what its failure
// names, and what the file reports once its last scenario ended.

import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { beforeAll, describe, expect, it } from 'vitest';

const FRONTEND = fileURLToPath(new URL('../..', import.meta.url));

interface Outcome {
	status: string;
	failure: string;
}
interface FileOutcome {
	scenarios: Record<string, Outcome>;
	/** What the file failed on outside any scenario: its `afterAll`. */
	fileFailure: string;
}

interface JsonReport {
	testResults: {
		name: string;
		message: string;
		assertionResults: { title: string; status: string; failureMessages: string[] }[];
	}[];
}

let report: Record<string, FileOutcome>;

const RUN_TIMEOUT_MS = 100_000;

interface Run {
	/** How the child ended: its exit code, or the signal that ended it. */
	ended: string;
	stderr: string;
}

/** Runs the fixtures; resolves once the child exits, however it exits,
 *  and kills it if it outlives the run's timeout. */
function runFixtures(outputFile: string): Promise<Run> {
	// Not the parent's vitest variables: the child is a run of its own.
	const env = Object.fromEntries(
		Object.entries(process.env).filter(([k]) => !k.startsWith('VITEST') && k !== 'NODE_ENV')
	);
	return new Promise((resolve, reject) => {
		const child = spawn(
			process.execPath,
			[
				join(FRONTEND, 'node_modules/vitest/vitest.mjs'),
				'run',
				'--config',
				'tests/acceptance-guard/vitest.fixtures.config.ts',
				'--reporter=json',
				`--outputFile=${outputFile}`
			],
			{ cwd: FRONTEND, env, stdio: ['ignore', 'ignore', 'pipe'] }
		);
		let stderr = '';
		child.stderr.on('data', (chunk) => (stderr += chunk));
		const timer = setTimeout(() => child.kill('SIGKILL'), RUN_TIMEOUT_MS);
		child.on('error', (err) => {
			clearTimeout(timer);
			reject(err);
		});
		child.on('close', (code, signal) => {
			clearTimeout(timer);
			const ended =
				signal === 'SIGKILL'
					? `killed after ${RUN_TIMEOUT_MS} ms`
					: signal
						? `signal ${signal}`
						: `exit ${code}`;
			resolve({ ended, stderr });
		});
	});
}

beforeAll(async () => {
	const dir = mkdtempSync(join(tmpdir(), 'acceptance-guard-'));
	try {
		const out = join(dir, 'report.json');
		const run = await runFixtures(out);
		const failed = (what: string) =>
			new Error(`the fixture run ${what} (${run.ended}):\n${run.stderr.slice(-4000)}`);
		if (!existsSync(out)) throw failed('wrote no report');
		let json: JsonReport;
		try {
			json = JSON.parse(readFileSync(out, 'utf8')) as JsonReport;
		} catch {
			throw failed('wrote a report that does not parse');
		}
		report = {};
		for (const file of json.testResults) {
			const scenarios: Record<string, Outcome> = {};
			for (const a of file.assertionResults) {
				scenarios[a.title] = { status: a.status, failure: a.failureMessages.join('\n') };
			}
			const name = file.name
				.split('/')
				.pop()!
				.replace(/\.fixture\.ts$/, '');
			report[name] = { scenarios, fileFailure: file.message ?? '' };
		}
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}, RUN_TIMEOUT_MS + 10_000);

const fixture = (name: string) => {
	const outcome = report[name];
	expect(outcome, `fixture ${name} ran`).toBeDefined();
	return outcome;
};
const passed = (o: Outcome | undefined) => {
	expect(o, 'scenario ran').toBeDefined();
	expect(o!.failure).toBe('');
	expect(o!.status).toBe('passed');
};
const failedOn = (o: Outcome | undefined, ...lines: RegExp[]) => {
	expect(o, 'scenario ran').toBeDefined();
	expect(o!.status).toBe('failed');
	for (const line of lines) expect(o!.failure).toMatch(line);
};
/** How many times `text` names `path` as a request line. */
const mentions = (text: string, path: string) =>
	text.split('\n').filter((l) => l.includes(`127.0.0.1:31337${path}`)).length;

describe('acceptance request guard', () => {
	it('passes a scenario whose requests were answered before its teardown', () => {
		const f = fixture('answered-in-time');
		passed(f.scenarios['answered before teardown']);
		expect(f.fileFailure).toBe('');
	});

	it('fails the scenario that sent a request still unanswered at its teardown, not the next', () => {
		const f = fixture('answered-after-teardown');
		failedOn(f.scenarios['leaves a slow request behind'], /GET \S+\/slow — still unanswered/);
		passed(f.scenarios['runs while the earlier request is answered']);
		expect(f.fileFailure).toBe('');
	});

	it('fails the last scenario of a file for its late request, and the file reports nothing more', () => {
		const f = fixture('answered-after-last');
		failedOn(
			f.scenarios['leaves a slow request behind as the last scenario'],
			/GET \S+\/slow — still unanswered/
		);
		expect(f.fileFailure).toBe('');
	});

	it('fails the scenario that left a request nothing answers, and does not wait for it', () => {
		const f = fixture('never-answered');
		failedOn(
			f.scenarios['leaves a request nothing will answer'],
			/GET \S+\/never — still unanswered/
		);
		passed(f.scenarios['runs after the unanswered request']);
		expect(f.fileFailure).toBe('');
	});

	it('passes a scenario that aborted its requests before teardown', () => {
		const f = fixture('abandoned');
		passed(f.scenarios['aborts its requests before teardown']);
		expect(f.fileFailure).toBe('');
	});

	it('fails the scenario that sent a request no handler answered, and only that one', () => {
		const f = fixture('undescribed');
		failedOn(f.scenarios['sends a request no handler answers'], /GET \S+\/nobody\b/);
		passed(f.scenarios['runs after the undescribed request']);
		expect(f.fileFailure).toBe('');
	});

	it('names a request sent after its scenario ended at the file’s end, failing no later scenario', () => {
		const f = fixture('sent-after-teardown');
		passed(f.scenarios['leaves a timer that sends later']);
		passed(f.scenarios['runs while the earlier timer sends']);
		expect(f.fileFailure).toMatch(
			/GET \S+\/ok — sent after "leaves a timer that sends later" had unmounted its page/
		);
	});

	it('fails one scenario on both its undescribed and its late request, leaving nothing behind', () => {
		const f = fixture('undescribed-and-late');
		const both = f.scenarios['sends an undescribed request and leaves a slow one behind'];
		passed(f.scenarios['runs after both']);
		failedOn(both, /GET \S+\/slow — still unanswered/, /GET \S+\/nobody — no handler/);
		expect(f.fileFailure).toBe('');
	});

	it('keeps an earlier scenario’s late request off a scenario failing on its own', () => {
		const f = fixture('late-lands-on-undescribed');
		passed(f.scenarios['leaves a timer that sends later']);
		passed(f.scenarios['runs after both']);
		const own = f.scenarios['sends an undescribed request while the timer sends'];
		expect(mentions(own?.failure ?? '', '/ok')).toBe(0);
		failedOn(own, /GET \S+\/nobody — no handler/);
		expect(f.fileFailure).toMatch(
			/GET \S+\/ok — sent after "leaves a timer that sends later" had unmounted its page/
		);
	});

	it('fails the scenario whose handler threw once, as a handler that threw', () => {
		const f = fixture('handler-threw');
		const threw = f.scenarios['meets a handler that throws'];
		expect(mentions(threw?.failure ?? '', '/boom')).toBe(1);
		failedOn(threw, /GET \S+\/boom — its handler threw/);
		passed(f.scenarios['runs after the handler threw']);
		expect(f.fileFailure).toBe('');
	});

	it('names a late request no handler answers once, at the file’s end', () => {
		const f = fixture('late-undescribed');
		passed(f.scenarios['leaves a timer that sends a request no handler answers']);
		passed(f.scenarios['runs while the earlier timer sends']);
		expect(mentions(f.fileFailure, '/nobody')).toBe(1);
		expect(f.fileFailure).toMatch(
			/GET \S+\/nobody — sent after "leaves a timer that sends a request no handler answers" had unmounted its page/
		);
	});

	it('names a request sent outside any scenario once, at the file’s end', () => {
		const f = fixture('outside-any');
		passed(f.scenarios['runs after a request sent from beforeAll']);
		expect(f.fileFailure).toMatch(/GET \S+\/nobody — sent outside any scenario/);
		expect(mentions(f.fileFailure, '/nobody')).toBe(1);
	});
});
