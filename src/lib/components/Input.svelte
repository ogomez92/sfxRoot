<script lang="ts">
	interface Props {
		type?: 'text' | 'search' | 'number' | 'password';
		value?: string;
		placeholder?: string;
		disabled?: boolean;
		id?: string;
		name?: string;
		oninput?: (event: Event) => void;
		onkeydown?: (event: KeyboardEvent) => void;
	}

	let {
		type = 'text',
		value = $bindable(''),
		placeholder = '',
		disabled = false,
		id,
		name,
		oninput,
		onkeydown
	}: Props = $props();
</script>

<input
	{type}
	{id}
	{name}
	{placeholder}
	{disabled}
	bind:value
	class="input"
	{oninput}
	{onkeydown}
/>

<style>
	.input {
		width: 100%;
		padding: 0.5rem 0.75rem;
		font-family: inherit;
		font-size: 0.9375rem;
		color: var(--color-text, #1e293b);
		background: var(--color-background, #ffffff);
		border: 1px solid var(--color-border, #e2e8f0);
		border-radius: 0.375rem;
		transition: all 0.15s ease;
	}

	.input::placeholder {
		color: var(--color-text-muted, #94a3b8);
	}

	.input:focus {
		outline: none;
		border-color: var(--color-primary, #3b82f6);
		box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
	}

	.input:disabled {
		opacity: 0.5;
		cursor: not-allowed;
		background: var(--color-surface, #f1f5f9);
	}
</style>
