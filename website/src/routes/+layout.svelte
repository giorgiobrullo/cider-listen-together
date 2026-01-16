<script lang="ts">
	import '../app.css';
	import 'overlayscrollbars/overlayscrollbars.css';
	import { OverlayScrollbarsComponent } from 'overlayscrollbars-svelte';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" type="image/png" href="/app-icon.png" />
	<link rel="apple-touch-icon" href="/app-icon.png" />
</svelte:head>

<OverlayScrollbarsComponent
	element="div"
	options={{
		scrollbars: {
			theme: 'os-theme-light',
			autoHide: 'leave',
			autoHideDelay: 400
		}
	}}
	class="h-screen"
>
	{@render children()}
</OverlayScrollbarsComponent>
