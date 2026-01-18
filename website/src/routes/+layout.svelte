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
	<link rel="icon" type="image/png" href="/app-icon.png" />
	<link rel="apple-touch-icon" href="/app-icon.png" />
</svelte:head>

{@render children()}
