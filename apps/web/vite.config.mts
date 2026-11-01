import { fileURLToPath } from 'node:url'
import { constants as zlibConstants } from 'node:zlib'

import { babelOptimizerPlugin as graphqlDocumentsPlugin } from '@graphql-codegen/client-preset'
import babel from '@rolldown/plugin-babel'
import tailwindcss from '@tailwindcss/vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import { defineConfig, type Plugin } from 'vite'
import { compression, defineAlgorithm } from 'vite-plugin-compression2'
import { VitePWA } from 'vite-plugin-pwa'
import reactFallbackThrottlePlugin from 'vite-plugin-react-fallback-throttle'
import tsconfigPaths from 'vite-plugin-tsconfig-paths'

// https://www.npmjs.com/package/vite-plugin-node-polyfills
import { name, version } from './package.json'

/**
 * Marks the generated GraphQL documents as side-effect free (`new TypedDocumentString(...)` is a
 * constructor call, which the bundler otherwise has to keep), so with the babel plugin below a
 * bundle carries only the documents its screens use. The codegen client preset doesn't forward
 * its `pureMagicComment` option, hence the transform here
 */
const pureGraphQLDocuments = (): Plugin => ({
	name: 'stump:pure-graphql-documents',
	enforce: 'pre',
	transform(code, id) {
		if (!id.replaceAll('\\', '/').endsWith('/packages/graphql/src/client/graphql.ts')) return
		return {
			code: code.replaceAll(
				'= new TypedDocumentString(',
				'= /*#__PURE__*/ new TypedDocumentString(',
			),
			map: null,
		}
	},
})

// https://vitejs.dev/config/
export default defineConfig({
	build: {
		assetsDir: './assets',
		manifest: true,
		outDir: '../dist',
		emptyOutDir: true,
		// Generated but not referenced by the bundles, so production stack traces (e.g. from
		// the error screen's copy button) can be mapped back to the source offline
		sourcemap: 'hidden',
		rollupOptions: {
			output: {
				// Translations and date-fns locales get a `locale-` prefix so the service worker can
				// leave them out of its precache (only the language in use is ever fetched)
				chunkFileNames(chunk) {
					const id = (chunk.facadeModuleId ?? '').replaceAll('\\', '/')
					const isLocale = /\/i18n\/src\/locales\/|\/date-fns\/locale\//.test(id)
					return isLocale ? 'assets/locale-[name]-[hash].js' : 'assets/[name]-[hash].js'
				},
				manualChunks(id) {
					const path = id.replaceAll('\\', '/')
					if (!path.includes('/node_modules/')) {
						return
					}

					if (path.includes('/node_modules/lucide-react/')) {
						return 'vendor-lucide'
					}
					if (path.includes('/node_modules/@tanstack/')) {
						return 'vendor-tanstack'
					}
					if (path.includes('/node_modules/lodash/')) {
						return 'vendor-lodash'
					}
					if (path.includes('/node_modules/framer-motion/')) {
						return 'vendor-framer'
					}
					if (path.includes('/node_modules/overlayscrollbars/')) {
						return 'vendor-overlayscrollbars'
					}
					if (
						path.includes('/node_modules/react/') ||
						path.includes('/node_modules/react-dom/') ||
						path.includes('/node_modules/scheduler/')
					) {
						return 'vendor-react'
					}
				},
			},
		},
	},
	clearScreen: false,
	define: {
		pkgJson: { name, version },
	},
	plugins: [
		pureGraphQLDocuments(),
		tailwindcss(),
		react(),
		babel({
			presets: [reactCompilerPreset()],
			// Turns each `graphql(...)` call into an import of that one document. Without it every
			// call goes through the generated map of all the app's documents, so the first load
			// of any page downloaded every query and fragment of the app
			plugins: [
				[
					graphqlDocumentsPlugin,
					{
						artifactDirectory: fileURLToPath(
							new URL('../../packages/graphql/src/client', import.meta.url),
						),
						gqlTagName: 'graphql',
					},
				],
			],
		}),
		tsconfigPaths(),
		compression({
			include: [/\.(js|mjs|json|css|html|svg|xml|wasm)$/i],
			exclude: [/\.(png|jpe?g|gif|webp|avif|woff2?|mp4|webm)$/i],
			threshold: 1024,
			algorithms: [
				defineAlgorithm('gzip', { level: 9 }),
				defineAlgorithm('brotliCompress', {
					params: {
						[zlibConstants.BROTLI_PARAM_QUALITY]: 11,
					},
				}),
			],
		}),
		VitePWA({
			// We manually register in src/index.tsx to add idle scheduling and script preflight checks.
			injectRegister: null,
			registerType: 'autoUpdate',
			devOptions: {
				enabled: false,
			},
			workbox: {
				inlineWorkboxRuntime: true,
				// The 64 translation/date locale chunks are fetched on demand and cached on first use
				globIgnores: ['**/node_modules/**/*', 'sw.js', 'workbox-*.js', '**/locale-*.js'],
				runtimeCaching: [
					{
						urlPattern: /\/assets\/locale-[^/]+\.js$/,
						handler: 'CacheFirst',
						options: {
							cacheName: 'stump-locales',
							expiration: { maxEntries: 8 },
						},
					},
				],
				navigateFallbackDenylist: [
					/^\/api(?:\/|$)/,
					/^\/opds(?:\/|$)/,
					/^\/kobo(?:\/|$)/,
					/^\/koreader(?:\/|$)/,
				],
				maximumFileSizeToCacheInBytes: 6 * 1024 * 1024, // 6MB
			},
			outDir: '../dist',
			base: '/',
			// TODO(pwa): Add more manifest definitions for better overall experience
			manifest: {
				id: 'stump',
				name: 'Stump PWA',
				short_name: 'Stump',
				theme_color: '#161719',
				icons: [
					{
						src: '/assets/favicon-16x16.png',
						sizes: '16x16',
						type: 'image/png',
					},
					{
						src: '/assets/favicon-192x192.png',
						sizes: '192x192',
						type: 'image/png',
					},
					{
						src: '/assets/favicon-512x512.png',
						sizes: '512x512',
						type: 'image/png',
						purpose: 'any maskable',
					},
				],
			},
			manifestFilename: 'assets/manifest.webmanifest',
		}),
		reactFallbackThrottlePlugin(),
	],
	publicDir: '../../../packages/browser/public',
	root: 'src',
	server: {
		port: 3000,
	},
})
