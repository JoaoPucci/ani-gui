import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';
import noHardcodedStrings from './tools/eslint-rules/no-hardcoded-strings.js';

// Local plugin scope so the i18n rule lives next to the project
// instead of as a separate npm package.
const local = {
	rules: { 'no-hardcoded-strings': noHardcodedStrings }
};

export default ts.config(
	js.configs.recommended,
	...ts.configs.recommended,
	...svelte.configs.recommended,
	{
		languageOptions: {
			globals: {
				...globals.browser,
				...globals.node
			}
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: {
				projectService: true,
				extraFileExtensions: ['.svelte'],
				parser: ts.parser,
				svelteConfig: undefined
			}
		},
		plugins: { local },
		rules: {
			// Custom rule — see tools/eslint-rules/no-hardcoded-strings.js.
			// Plain .ts modules under src/ get it from the block below.
			'local/no-hardcoded-strings': 'error'
		}
	},
	{
		// Plain TypeScript modules carry copy too: a label table or a
		// credits entry a component only renders. In these files only
		// the rule's JavaScript-literal check can fire (there is no
		// template). Tests are left out — their strings are fixtures.
		files: ['src/**/*.ts'],
		ignores: ['src/**/*.test.ts', 'src/**/*.spec.ts'],
		plugins: { local },
		rules: {
			'local/no-hardcoded-strings': 'error'
		}
	},
	{
		// The acceptance tier's request guard cannot tell a request MSW
		// sends on to a port that refuses it from one a handler
		// answered (see tests/acceptance/setup.ts), so the tier does
		// not send requests on at all.
		files: ['tests/acceptance/**/*.ts'],
		rules: {
			'no-restricted-imports': [
				'error',
				{
					paths: [
						{
							name: 'msw',
							importNames: ['passthrough', 'bypass'],
							message:
								'The acceptance tier answers every request; a request sent on to the network can slip past its guard.'
						}
					]
				}
			]
		}
	},
	{
		// `src/lib/paraglide/` is the typed-TS output of the Paraglide
		// compiler — it ships with `/* eslint-disable */` headers
		// per file but recent eslint flags those as unused-disable
		// when there's nothing to disable. Skip the directory wholesale
		// so paraglide-js upgrades don't churn lint output.
		ignores: [
			'.svelte-kit/',
			'build/',
			'coverage/',
			'dist/',
			'node_modules/',
			'src/lib/bindings/',
			'src/lib/paraglide/'
		]
	}
);
