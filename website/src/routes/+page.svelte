<script lang="ts">
	import { MetaTags } from 'svelte-meta-tags';
	import {
		IconBrandApple,
		IconBrandWindows,
		IconDownload,
		IconRefresh,
		IconNetwork,
		IconHash,
		IconDeviceDesktop,
		IconPlayerPlay,
		IconLock,
		IconBrandGithub,
		IconCopy,
		IconCheck,
		IconMail
	} from '@tabler/icons-svelte';

	let copied = $state(false);
	const brewCommand = 'brew install giorgiobrullo/tap/cider-together';

	const siteUrl = 'https://cidertogether.app';
	const title = 'Listen Together - Sync Cider playback with friends';
	const description = 'Listen to Apple Music together with friends in real-time. P2P sync via libp2p. Available for macOS and Windows.';

	// Obfuscated email - split to avoid bot scraping
	const emailParts = ['contact', 'cidertogether', 'app'];
	const email = $derived(`${emailParts[0]}@${emailParts[1]}.${emailParts[2]}`);

	async function copyToClipboard() {
		await navigator.clipboard.writeText(brewCommand);
		copied = true;
		setTimeout(() => (copied = false), 2000);
	}

	function scrollToDownload(e: MouseEvent) {
		e.preventDefault();
		document.getElementById('download')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	const features = [
		{
			icon: IconRefresh,
			title: 'Real-Time Sync',
			description: 'Sub-second synchronization keeps everyone perfectly in time, no matter the distance.'
		},
		{
			icon: IconNetwork,
			title: 'Peer-to-Peer',
			description: 'Direct connections via libp2p. Your music syncs between devices, not through the cloud.'
		},
		{
			icon: IconHash,
			title: 'Simple Room Codes',
			description: 'Share an 8-character code and your friends are in. It\'s that easy.'
		},
		{
			icon: IconDeviceDesktop,
			title: 'Native Apps',
			description: 'Beautiful native experiences on macOS and Windows, built for performance.'
		},
		{
			icon: IconPlayerPlay,
			title: 'Host Controls',
			description: 'The host controls playback. Play, pause, skip—everyone follows along.'
		},
		{
			icon: IconLock,
			title: 'Private Sessions',
			description: 'Your listening sessions stay between you and your friends. Always.'
		}
	];
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
	<!-- Background layers -->
	<div class="fixed inset-0 -z-10 overflow-hidden">
		<!-- Base gradient -->
		<div class="absolute inset-0 bg-gradient-to-b from-[#0d0d0d] via-[#0a0a0a] to-[#050505]"></div>

		<!-- Animated glow orbs -->
		<div class="absolute top-[10%] left-[20%] w-[500px] h-[500px] bg-accent/8 rounded-full blur-[150px] animate-pulse-slow"></div>
		<div class="absolute top-[40%] right-[10%] w-[400px] h-[400px] bg-orange-500/5 rounded-full blur-[120px] animate-pulse-slower"></div>
		<div class="absolute bottom-[20%] left-[30%] w-[600px] h-[600px] bg-amber-500/5 rounded-full blur-[180px]"></div>

		<!-- Subtle grid overlay -->
		<div class="absolute inset-0 bg-[linear-gradient(rgba(255,255,255,0.01)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.01)_1px,transparent_1px)] bg-[size:100px_100px]"></div>

		<!-- Noise texture -->
		<div class="absolute inset-0 opacity-[0.015]" style="background-image: url('data:image/svg+xml,%3Csvg viewBox=%270 0 256 256%27 xmlns=%27http://www.w3.org/2000/svg%27%3E%3Cfilter id=%27noise%27%3E%3CfeTurbulence type=%27fractalNoise%27 baseFrequency=%270.9%27 numOctaves=%274%27 stitchTiles=%27stitch%27/%3E%3C/filter%3E%3Crect width=%27100%25%27 height=%27100%25%27 filter=%27url(%23noise)%27/%3E%3C/svg%3E');"></div>
	</div>

	<!-- Hero Section -->
	<section class="relative px-6 pt-24 pb-32 md:pt-36 md:pb-44">
		<div class="max-w-4xl mx-auto text-center">
			<!-- Logo/Icon -->
			<div class="mb-10 animate-fade-in-up">
				<div class="relative inline-block">
					<img src="/app-icon.png" alt="Listen Together" class="w-32 h-32 mx-auto rounded-[2rem] shadow-2xl animate-float relative z-10" />
					<!-- Glow behind icon -->
					<div class="absolute inset-0 w-32 h-32 mx-auto bg-accent/40 rounded-[2rem] blur-2xl -z-10 animate-pulse-slow"></div>
				</div>
			</div>

			<!-- Title -->
			<h1 class="text-5xl md:text-7xl font-bold tracking-tight mb-6 animate-fade-in-up animation-delay-100 opacity-0">
				Listen <span class="gradient-text">Together</span>
			</h1>

			<!-- Subtitle -->
			<p class="text-xl md:text-2xl text-white/50 max-w-2xl mx-auto mb-12 leading-relaxed animate-fade-in-up animation-delay-200 opacity-0">
				Sync your Cider playback with friends in real-time.<br class="hidden sm:block" />
				Low latency, peer-to-peer, just music.
			</p>

			<!-- CTA Buttons -->
			<div class="flex flex-col sm:flex-row gap-4 justify-center animate-fade-in-up animation-delay-300 opacity-0">
				<a href="#download" onclick={scrollToDownload} class="group inline-flex items-center justify-center gap-2.5 px-8 py-4 bg-white text-black font-semibold rounded-2xl hover:bg-white/90 transition-all duration-300 hover:scale-[1.02] hover:shadow-lg hover:shadow-white/10">
					<IconBrandApple class="w-5 h-5" stroke={1.5} />
					Download for macOS
				</a>
				<a href="#download" onclick={scrollToDownload} class="group inline-flex items-center justify-center gap-2.5 px-8 py-4 glass rounded-2xl font-semibold transition-all duration-300 hover:bg-white/10 hover:scale-[1.02] hover:border-white/20">
					<IconBrandWindows class="w-5 h-5" stroke={1.5} />
					Download for Windows
				</a>
			</div>

			<!-- Screenshot -->
			<div class="mt-20 animate-fade-in-up animation-delay-400 opacity-0">
				<div class="relative mx-auto inline-block">
					<!-- Screenshot glow -->
					<div class="absolute -inset-4 bg-gradient-to-b from-accent/20 via-accent/5 to-transparent rounded-3xl blur-2xl"></div>
					<img src="/screenshot.png" alt="Listen Together app screenshot" class="relative max-h-[500px] mx-auto rounded-2xl shadow-2xl ring-1 ring-white/10" />
				</div>
			</div>
		</div>
	</section>

	<!-- Features Section -->
	<section class="relative px-6 py-28" id="features">
		<div class="max-w-6xl mx-auto">
			<div class="text-center mb-16">
				<h2 class="text-3xl md:text-5xl font-bold mb-5">Everything you need</h2>
				<p class="text-white/40 text-lg max-w-xl mx-auto">
					Built with modern P2P technology for the best listening experience
				</p>
			</div>

			<div class="grid md:grid-cols-2 lg:grid-cols-3 gap-5">
				{#each features as feature, i}
					<div class="group glass rounded-2xl p-7 transition-all duration-300 hover:bg-white/[0.03] hover:border-white/20 hover:-translate-y-1">
						<div class="w-12 h-12 mb-5 rounded-xl bg-gradient-to-br from-accent/20 to-accent/5 flex items-center justify-center text-accent ring-1 ring-accent/20 group-hover:ring-accent/40 transition-all">
							<feature.icon class="w-6 h-6" stroke={1.5} />
						</div>
						<h3 class="text-lg font-semibold mb-2 group-hover:text-accent transition-colors">{feature.title}</h3>
						<p class="text-white/40 text-sm leading-relaxed">{feature.description}</p>
					</div>
				{/each}
			</div>
		</div>
	</section>

	<!-- How it works -->
	<section class="relative px-6 py-28">
		<!-- Section background -->
		<div class="absolute inset-0 bg-gradient-to-b from-transparent via-white/[0.02] to-transparent"></div>

		<div class="relative max-w-4xl mx-auto text-center">
			<h2 class="text-3xl md:text-5xl font-bold mb-20">Get started in seconds</h2>

			<div class="grid md:grid-cols-3 gap-12 md:gap-8">
				{#each [
					{ num: '1', title: 'Create a Room', desc: 'Host creates a room and gets a unique 8-character code' },
					{ num: '2', title: 'Share the Code', desc: 'Send the code to friends—that\'s all they need' },
					{ num: '3', title: 'Listen Together', desc: 'Everyone\'s music syncs automatically. Enjoy!' }
				] as step, i}
					<div class="relative group">
						<!-- Connector line -->
						{#if i < 2}
							<div class="hidden md:block absolute top-8 left-[60%] w-[80%] h-px bg-gradient-to-r from-white/10 to-transparent -z-10"></div>
						{/if}
						<div class="relative z-10 w-16 h-16 mx-auto mb-6 rounded-2xl bg-bg-dark flex items-center justify-center text-2xl font-bold text-accent ring-1 ring-accent/20 group-hover:ring-accent/40 group-hover:scale-110 transition-all duration-300">
							<div class="absolute inset-0 rounded-2xl bg-gradient-to-br from-accent/20 to-accent/5"></div>
							<span class="relative">{step.num}</span>
						</div>
						<h3 class="text-xl font-semibold mb-3">{step.title}</h3>
						<p class="text-white/40 text-sm">{step.desc}</p>
					</div>
				{/each}
			</div>
		</div>
	</section>

	<!-- Download Section -->
	<section class="relative px-6 py-28" id="download">
		<div class="max-w-4xl mx-auto">
			<div class="text-center mb-16">
				<h2 class="text-3xl md:text-5xl font-bold mb-5">Get the app</h2>
				<p class="text-white/40 text-lg">Available for macOS and Windows</p>
			</div>

			<!-- macOS -->
			<div class="mb-6">
				<div class="flex items-center gap-3 mb-4">
					<IconBrandApple class="w-6 h-6 text-white/60" stroke={1.5} />
					<h3 class="text-lg font-semibold">macOS</h3>
					<span class="text-white/30 text-sm">26 Tahoe or later</span>
				</div>

				<div class="flex flex-col sm:flex-row gap-3">
					<!-- Homebrew -->
					<div class="flex-1 flex items-center gap-2 bg-white/5 rounded-xl px-4 py-3 ring-1 ring-white/10">
						<img src="/homebrew.svg" alt="Homebrew" class="w-5 h-5 opacity-60" />
						<code class="flex-1 font-mono text-sm text-white/60 overflow-x-auto">{brewCommand}</code>
						<button
							onclick={copyToClipboard}
							class="flex-shrink-0 p-2 rounded-lg hover:bg-white/10 transition-colors"
							title="Copy to clipboard"
						>
							{#if copied}
								<IconCheck class="w-4 h-4 text-green-400" stroke={2} />
							{:else}
								<IconCopy class="w-4 h-4 text-white/40" stroke={1.5} />
							{/if}
						</button>
					</div>

					<!-- DMG download -->
					<a
						href="https://github.com/giorgiobrullo/cider-listen-together/releases/latest"
						target="_blank"
						rel="noopener"
						class="inline-flex items-center justify-center gap-2 px-6 py-3 bg-white text-black font-medium rounded-xl hover:bg-white/90 transition-colors"
					>
						<IconDownload class="w-4 h-4" stroke={2} />
						Download .dmg
					</a>
				</div>
			</div>

			<!-- Divider -->
			<div class="h-px bg-white/5 my-8"></div>

			<!-- Windows -->
			<div>
				<div class="flex items-center gap-3 mb-4">
					<IconBrandWindows class="w-6 h-6 text-white/60" stroke={1.5} />
					<h3 class="text-lg font-semibold">Windows</h3>
					<span class="text-white/30 text-sm">10 or later</span>
					<span class="text-xs text-accent/80 bg-accent/10 px-2 py-0.5 rounded-full">Coming soon</span>
				</div>

				<a
					href="https://github.com/giorgiobrullo/cider-listen-together/releases"
					target="_blank"
					rel="noopener"
					class="inline-flex items-center justify-center gap-2 px-6 py-3 bg-white/10 font-medium rounded-xl ring-1 ring-white/10 hover:bg-white/15 hover:ring-white/20 transition-all opacity-60"
				>
					<IconBrandGithub class="w-4 h-4" stroke={2} />
					View releases
				</a>
			</div>

			<!-- Requirements note -->
			<div class="mt-12 pt-8 border-t border-white/5 text-center">
				<p class="text-white/30 text-sm">
					Requires <a href="https://cider.sh" target="_blank" rel="noopener" class="text-accent hover:underline underline-offset-4">Cider</a> and an Apple Music subscription
				</p>
			</div>
		</div>
	</section>

	<!-- Footer -->
	<footer class="relative px-6 py-14 border-t border-white/5">
		<div class="max-w-4xl mx-auto">
			<div class="flex flex-col md:flex-row justify-between items-center gap-6 mb-8">
				<div class="flex items-center gap-3">
					<img src="/app-icon.png" alt="Listen Together" class="w-9 h-9 rounded-xl" />
					<span class="font-semibold text-white/90">Listen Together</span>
				</div>
				<div class="flex items-center gap-6 text-white/40 text-sm">
					<a href="https://github.com/giorgiobrullo/cider-listen-together" target="_blank" rel="noopener" class="hover:text-white transition-colors flex items-center gap-2">
						<IconBrandGithub class="w-4 h-4" stroke={1.5} />
						GitHub
					</a>
					<a href={`mailto:${email}`} class="hover:text-white transition-colors flex items-center gap-2">
						<IconMail class="w-4 h-4" stroke={1.5} />
						Contact
					</a>
					<span>&copy; {new Date().getFullYear()} Giorgio Brullo</span>
				</div>
			</div>
			<p class="text-center text-white/20 text-xs">
				This project is not affiliated with, endorsed by, or associated with <a href="https://cider.sh" target="_blank" rel="noopener" class="hover:text-white/40 transition-colors">Cider Collective</a> or Apple Inc.
			</p>
		</div>
	</footer>
</div>
