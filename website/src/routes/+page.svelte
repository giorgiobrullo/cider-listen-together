<script lang="ts">
	import { MetaTags } from 'svelte-meta-tags';
	import {
		IconBrandApple,
		IconBrandWindows,
		IconDownload,
		IconBrandGithub,
		IconCopy,
		IconCheck,
		IconMail
	} from '@tabler/icons-svelte';
	import { onMount } from 'svelte';

	let copied = $state(false);
	let scrollY = $state(0);

	onMount(() => {
		const handleScroll = () => {
			scrollY = window.scrollY;
		};
		window.addEventListener('scroll', handleScroll, { passive: true });
		return () => window.removeEventListener('scroll', handleScroll);
	});
	const brewCommand = 'brew install giorgiobrullo/tap/cider-together';

	const siteUrl = 'https://cidertogether.app';
	const title = 'Listen Together - Sync Cider playback with friends';
	const description = 'Listen to Apple Music together with friends in real-time. P2P sync via libp2p. Available for macOS and Windows.';

	const emailParts = ['contact', 'cidertogether', 'app'];
	const email = $derived(`${emailParts[0]}@${emailParts[1]}.${emailParts[2]}`);

	async function copyToClipboard() {
		await navigator.clipboard.writeText(brewCommand);
		copied = true;
		setTimeout(() => (copied = false), 2000);
	}
</script>

<MetaTags
	{title}
	{description}
	canonical={siteUrl}
	openGraph={{
		type: 'website',
		url: siteUrl,
		title,
		description,
		siteName: 'Listen Together',
		images: [
			{
				url: `${siteUrl}/screenshot.png`,
				width: 1200,
				height: 630,
				alt: 'Listen Together app screenshot'
			}
		]
	}}
	twitter={{
		cardType: 'summary_large_image',
		title,
		description,
		image: `${siteUrl}/screenshot.png`,
		imageAlt: 'Listen Together app screenshot'
	}}
	additionalMetaTags={[
		{ name: 'keywords', content: 'Apple Music, Cider, listen together, music sync, P2P, peer-to-peer, macOS, Windows' },
		{ name: 'author', content: 'Giorgio Brullo' },
		{ name: 'theme-color', content: '#f97316' }
	]}
/>

<div class="min-h-screen relative">
	<!-- Background -->
	<div class="fixed inset-0 -z-10 bg-[#09090b]">
		<!-- Ambient glow from top -->
		<div
			class="absolute inset-0 pointer-events-none"
			style="background: radial-gradient(ellipse 100% 70% at 50% -20%, rgba(249, 115, 22, 0.15), transparent 70%);"
		></div>

		<!-- Secondary glow that shifts slightly with scroll -->
		<div
			class="absolute inset-0 pointer-events-none will-change-transform"
			style="
				background: radial-gradient(ellipse 80% 50% at 50% 20%, rgba(251, 146, 60, 0.06), transparent 60%);
				transform: translateY({scrollY * 0.1}px) scale({1 + scrollY * 0.0002});
			"
		></div>
	</div>

	<div class="max-w-2xl mx-auto px-6 py-24 md:py-32">
		<!-- Header -->
		<header class="flex items-center gap-4 mb-20">
			<img src="/app-icon.png" alt="Listen Together" class="w-14 h-14 rounded-2xl shadow-lg" />
			<div>
				<h1 class="text-xl font-semibold">Listen Together</h1>
				<p class="text-white/40 text-sm">for Cider</p>
			</div>
		</header>

		<!-- Main content -->
		<main>
			<p class="text-3xl md:text-4xl font-medium leading-snug mb-8 text-white/90">
				Sync your music with friends.<br />
				<span class="text-white/40">Share a code, listen together.</span>
			</p>

			<p class="text-white/50 text-lg leading-relaxed mb-12 max-w-lg">
				A companion app for <a href="https://cider.sh" target="_blank" rel="noopener" class="text-accent hover:underline underline-offset-4">Cider</a> that syncs playback between you and your friends using peer-to-peer connections. One person hosts, everyone else joins with a simple 8-character code.
			</p>

			<!-- Screenshot -->
			<div class="mb-16 -mx-6 md:mx-0">
				<img
					src="/screenshot.png"
					alt="Listen Together app"
					class="w-auto max-h-[400px] mx-auto rounded-xl md:rounded-2xl shadow-2xl ring-1 ring-white/10"
				/>
			</div>

			<!-- Download -->
			<section class="mb-20">
				<h2 class="text-sm font-medium text-white/30 uppercase tracking-wider mb-6">Download</h2>

				<!-- macOS -->
				<div class="mb-8">
					<div class="flex items-center gap-2 mb-3">
						<IconBrandApple class="w-5 h-5 text-white/50" stroke={1.5} />
						<span class="font-medium">macOS</span>
						<span class="text-white/30 text-sm">Tahoe or later</span>
					</div>

					<div class="space-y-3">
						<div class="flex items-center gap-2 bg-white/5 rounded-lg px-4 py-3 ring-1 ring-white/10">
							<img src="/homebrew.svg" alt="Homebrew" class="w-4 h-4 opacity-50" />
							<code class="flex-1 font-mono text-sm text-white/50 overflow-x-auto">{brewCommand}</code>
							<button
								onclick={copyToClipboard}
								class="p-1.5 rounded hover:bg-white/10 transition-colors"
								title="Copy"
							>
								{#if copied}
									<IconCheck class="w-4 h-4 text-green-400" stroke={2} />
								{:else}
									<IconCopy class="w-4 h-4 text-white/40" stroke={1.5} />
								{/if}
							</button>
						</div>

						<a
							href="https://github.com/giorgiobrullo/cider-listen-together/releases/latest"
							target="_blank"
							rel="noopener"
							class="inline-flex items-center gap-2 px-4 py-2.5 bg-white text-black text-sm font-medium rounded-lg hover:bg-white/90 transition-colors"
						>
							<IconDownload class="w-4 h-4" stroke={2} />
							Download .dmg
						</a>
					</div>
				</div>

				<!-- Windows -->
				<div>
					<div class="flex items-center gap-2 mb-3">
						<IconBrandWindows class="w-5 h-5 text-white/50" stroke={1.5} />
						<span class="font-medium">Windows</span>
						<span class="text-white/30 text-sm">10 or later</span>
						<span class="text-xs text-accent bg-accent/10 px-2 py-0.5 rounded-full">Soon</span>
					</div>

					<a
						href="https://github.com/giorgiobrullo/cider-listen-together/releases"
						target="_blank"
						rel="noopener"
						class="inline-flex items-center gap-2 px-4 py-2.5 text-sm font-medium rounded-lg ring-1 ring-white/10 hover:bg-white/5 transition-colors text-white/60"
					>
						<IconBrandGithub class="w-4 h-4" stroke={2} />
						View releases
					</a>
				</div>
			</section>

			<!-- Links -->
			<div class="flex flex-wrap gap-6 text-sm text-white/40">
				<a href="https://github.com/giorgiobrullo/cider-listen-together" target="_blank" rel="noopener" class="hover:text-white transition-colors flex items-center gap-1.5">
					<IconBrandGithub class="w-4 h-4" stroke={1.5} />
					Source
				</a>
				<a href={`mailto:${email}`} class="hover:text-white transition-colors flex items-center gap-1.5">
					<IconMail class="w-4 h-4" stroke={1.5} />
					Contact
				</a>
			</div>
		</main>

		<!-- Footer -->
		<footer class="mt-24 pt-8 border-t border-white/5 text-white/20 text-xs space-y-2">
			<p>&copy; {new Date().getFullYear()} Giorgio Brullo</p>
			<p>
				Not affiliated with <a href="https://cider.sh" target="_blank" rel="noopener" class="hover:text-white/40 transition-colors">Cider Collective</a> or Apple Inc.
			</p>
		</footer>
	</div>
</div>
