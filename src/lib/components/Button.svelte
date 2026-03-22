<script lang="ts">
	interface Props {
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
		size?: 'sm' | 'md' | 'lg';
		disabled?: boolean;
		type?: 'button' | 'submit' | 'reset';
		onclick?: (event: MouseEvent) => void;
		children?: import('svelte').Snippet;
	}

	let {
		variant = 'secondary',
		size = 'md',
		disabled = false,
		type = 'button',
		onclick,
		children
	}: Props = $props();
</script>

<button
	{type}
	{disabled}
	class="button button-{variant} button-{size}"
	{onclick}
>
	{#if children}
		{@render children()}
	{/if}
</button>

<style>
	.button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		font-family: inherit;
		font-weight: 500;
		border-radius: 0.375rem;
		border: 1px solid transparent;
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.button:focus-visible {
		outline: 2px solid var(--color-primary, #3b82f6);
		outline-offset: 2px;
	}

	/* Sizes */
	.button-sm {
		padding: 0.375rem 0.75rem;
		font-size: 0.875rem;
	}

	.button-md {
		padding: 0.5rem 1rem;
		font-size: 0.9375rem;
	}

	.button-lg {
		padding: 0.75rem 1.5rem;
		font-size: 1rem;
	}

	/* Variants */
	.button-primary {
		background: var(--color-primary, #3b82f6);
		color: white;
		border-color: var(--color-primary, #3b82f6);
	}

	.button-primary:hover:not(:disabled) {
		background: var(--color-primary-hover, #2563eb);
		border-color: var(--color-primary-hover, #2563eb);
	}

	.button-secondary {
		background: var(--color-surface, #f1f5f9);
		color: var(--color-text, #1e293b);
		border-color: var(--color-border, #e2e8f0);
	}

	.button-secondary:hover:not(:disabled) {
		background: var(--color-surface-hover, #e2e8f0);
	}

	.button-ghost {
		background: transparent;
		color: var(--color-text, #1e293b);
	}

	.button-ghost:hover:not(:disabled) {
		background: var(--color-surface, #f1f5f9);
	}

	.button-danger {
		background: var(--color-danger, #ef4444);
		color: white;
		border-color: var(--color-danger, #ef4444);
	}

	.button-danger:hover:not(:disabled) {
		background: var(--color-danger-hover, #dc2626);
		border-color: var(--color-danger-hover, #dc2626);
	}
</style>
