<script lang="ts">
	interface Props {
		value: number; // 0-100
		max?: number;
		label?: string;
		showPercent?: boolean;
	}

	let { value, max = 100, label = 'Progress', showPercent = true }: Props = $props();

	const percent = $derived(max > 0 ? Math.round((value / max) * 100) : 0);
</script>

<div class="progress-container">
	{#if label}
		<div class="progress-label">
			<span>{label}</span>
			{#if showPercent}
				<span class="progress-percent">{percent}%</span>
			{/if}
		</div>
	{/if}
	<div
		class="progress-bar"
		role="progressbar"
		aria-valuenow={value}
		aria-valuemin={0}
		aria-valuemax={max}
		aria-label={label}
	>
		<div class="progress-fill" style:width="{percent}%"></div>
	</div>
</div>

<style>
	.progress-container {
		width: 100%;
	}

	.progress-label {
		display: flex;
		justify-content: space-between;
		margin-bottom: 0.5rem;
		font-size: 0.875rem;
		color: var(--color-text, #1e293b);
	}

	.progress-percent {
		color: var(--color-text-muted, #64748b);
		font-variant-numeric: tabular-nums;
	}

	.progress-bar {
		height: 0.5rem;
		background: var(--color-surface, #e2e8f0);
		border-radius: 9999px;
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		background: var(--color-primary, #3b82f6);
		border-radius: 9999px;
		transition: width 0.3s ease;
	}
</style>
