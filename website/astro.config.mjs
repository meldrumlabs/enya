// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightLinksValidator from 'starlight-links-validator';
import starlightLlmsTxt from 'starlight-llms-txt';
import mermaid from 'astro-mermaid';

const site = 'https://enya.build';
const ogUrl = new URL('/img/enya.png', site).href;
const ogImageAlt = 'Enya';

// https://astro.build/config
export default defineConfig({
	site,
	integrations: [
		mermaid(),
		starlight({
			title: 'Enya',
			description: 'Enya',
			favicon: '/favicon.ico',
			social: [],
			components: {
				// Override the default header to remove theme and i18n selectors.
				Header: './src/components/Header.astro',
				// Override the default social icons to configure size and color behavior.
				SocialIcons: './src/components/SocialIcons.astro',
				// Override the default page frame to add the footer.
				PageFrame: './src/components/PageFrame.astro',
				// Override the default theme provider to ensure light mode is always enabled.
				ThemeProvider: './src/components/ThemeProvider.astro',
				Hero: './src/components/Hero.astro',
			},
			customCss: ['./src/styles/custom.css'],
			expressiveCode: {
				themes: ['github-dark'],
				styleOverrides: {
					borderColor: '#404040',
					borderRadius: '0.5rem',
					frames: {
						frameBoxShadowCssValue: 'none',
					},
				},
			},
			head: [
				{
					tag: 'meta',
					attrs: { property: 'og:image', content: ogUrl },
				},
				{
					tag: 'meta',
					attrs: { property: 'og:image:alt', content: ogImageAlt },
				},
			],
			plugins: [
				starlightLinksValidator(),
				starlightLlmsTxt(),
			],
			sidebar: [
				{ label: 'Getting Started', slug: 'docs/getting-started/introduction' },
				{ label: 'Keyboard Reference', slug: 'docs/editor/keyboard-reference' },
			],
		}),
	],
	vite: {
		assetsInclude: ['**/*.riv'],
	},
});
