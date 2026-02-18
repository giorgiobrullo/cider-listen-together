<script lang="ts">
	import '../app.css';
	import Lenis from 'lenis';
	import { onMount } from 'svelte';

	let { children } = $props();

	onMount(() => {
		const lenis = new Lenis({
			lerp: 0.1,
			smoothWheel: true
		});

		function raf(time: number) {
			lenis.raf(time);
			requestAnimationFrame(raf);
		}
		requestAnimationFrame(raf);

		return () => lenis.destroy();
	});
</script>

<svelte:head>
	<link rel="icon" href="/favicon.ico" sizes="32x32" />
	<link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png" />
	<link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
</svelte:head>

{@render children()}
