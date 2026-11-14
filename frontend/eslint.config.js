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

// The tests' fake backend answers (`src/lib/testing/`) are test code:
// app code imports nothing through a path segment named `testing` — a
// static import or re-export, a dynamic `import()` of a string or a
// template, or an `import.meta.glob` pattern. (`no-restricted-imports`
// sees only the first, so these are syntax rules.)
const TESTING_SEGMENT = '/(^|\\/)testing(\\/|\\.|$)/';
const TESTING_MESSAGE = '$lib/testing holds test fakes; app code does not import it.';
const TESTING_ONLY_SYNTAX = [
	{
		selector: `:matches(ImportDeclaration, ExportAllDeclaration, ExportNamedDeclaration, ImportExpression)[source.value=${TESTING_SEGMENT}]`,
		message: TESTING_MESSAGE
	},
	{
		selector: `ImportExpression > TemplateLiteral > TemplateElement[value.raw=${TESTING_SEGMENT}]`,
		message: TESTING_MESSAGE
	},
	{
		selector: `CallExpression[callee.object.meta.name='import'][callee.property.name=/^glob/] :matches(Literal[value=${TESTING_SEGMENT}], TemplateElement[value.raw=${TESTING_SEGMENT}])`,
		message: TESTING_MESSAGE
	}
];

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
		// Reduced motion is honoured by construction: every piece of
		// script motion goes through `src/lib/motion/`, which reads the
		// preference. Outside it, these are the forms that start motion
		// without asking. `tests/lint/reduced-motion.test.ts` runs these
		// rules over fixtures and says what they do not cover.
		files: ['src/**/*.{ts,js,svelte}'],
		ignores: ['src/lib/motion/**', 'src/**/*.test.ts', 'src/**/*.spec.ts'],
		rules: {
			'no-restricted-imports': [
				'error',
				{
					paths: ['svelte/transition', 'svelte/animate', 'svelte/motion', 'lottie-web'].map(
						(name) => ({
							name,
							message:
								'Motion goes through $lib/motion, which stands it still under reduced motion.'
						})
					)
				}
			],
			'no-restricted-syntax': [
				'error',
				...TESTING_ONLY_SYNTAX,
				{
					selector: "ImportExpression[source.value='lottie-web']",
					message:
						'Load Lottie through $lib/motion/lottie, which holds it still under reduced motion.'
				},
				{
					selector:
						"CallExpression[callee.type='MemberExpression'][callee.property.name='animate']",
					message:
						'Start a Web Animation through $lib/motion, which stands it still under reduced motion.'
				},
				{
					selector: "Property[key.name='behavior'][value.value='smooth']",
					message:
						'Take a scroll behaviour from scrollBehavior() in $lib/motion/scroll, which jumps under reduced motion.'
				},
				{
					selector: 'Literal[value=/prefers-reduced-motion/]',
					message:
						'Read the motion preference from reducedMotion in $lib/motion/reduced-motion.svelte.'
				},
				{
					selector: 'TemplateElement[value.raw=/prefers-reduced-motion/]',
					message:
						'Read the motion preference from reducedMotion in $lib/motion/reduced-motion.svelte.'
				}
			]
		}
	},
	{
		// `$lib/testing` holds the tests' fake backend answers; the app
		// never imports it. (Every other app file gets the same pattern
		// from the reduced-motion block above, beside its own list.)
		files: ['src/lib/motion/**/*.{ts,js,svelte}'],
		ignores: ['src/**/*.test.ts', 'src/**/*.spec.ts'],
		rules: {
			'no-restricted-syntax': ['error', ...TESTING_ONLY_SYNTAX]
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
		// A fake backend answer names the route's type, so the compiler
		// holds it to what the backend sends: `HttpResponse.json<T>(...)`,
		// or a helper from `tests/answers.ts` (an error's envelope, an
		// answer wrong on purpose). `tests/lint/typed-answers.test.ts`
		// runs these over fixtures and says what they do not see.
		files: ['tests/**/*.ts', 'src/**/*.test.ts', 'src/**/*.spec.ts'],
		rules: {
			'no-restricted-syntax': [
				'error',
				{
					selector:
						"CallExpression[callee.type='MemberExpression'][callee.object.name='HttpResponse'][callee.property.name='json']:not([typeArguments])",
					message:
						"Name the route's type, HttpResponse.json<T>(…), or answer through tests/answers (errorAnswer, malformedAnswer)."
				},
				{
					selector:
						"CallExpression[callee.object.name='HttpResponse'][callee.property.name='json'] > TSTypeParameterInstantiation :matches(TSUnknownKeyword, TSAnyKeyword, TSObjectKeyword, TSTypeLiteral:not([members.0]))",
					message:
						"That type holds the body to nothing: name the route's type, or mark a wrong answer with malformedAnswer from tests/answers."
				},
				{
					selector:
						"CallExpression[callee.object.name='HttpResponse'][callee.property.name='json'] > TSTypeParameterInstantiation > :matches(TSNeverKeyword, TSTypeReference[typeName.name=/^(JsonBodyType|Object)$/])",
					message:
						"That type holds the body to nothing: name the route's type, or mark a wrong answer with malformedAnswer from tests/answers."
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
		ignores: ['.svelte-kit/', 'build/', 'coverage/', 'dist/', 'node_modules/', 'src/lib/paraglide/']
	}
);
