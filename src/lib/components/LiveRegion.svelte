<script lang="ts">
	interface Props {
		message?: string;
		politeness?: 'polite' | 'assertive';
	}

	let { message = '', politeness = 'polite' }: Props = $props();
</script>

<div class="sr-only" aria-live={politeness} aria-atomic="true">
	{message}
</div>

<style>
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
