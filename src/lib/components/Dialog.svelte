<script lang="ts">
	interface Props {
		open?: boolean;
		title?: string;
		onclose?: () => void;
		children?: import('svelte').Snippet;
		actions?: import('svelte').Snippet;
	}

	let { open = $bindable(false), title, onclose, children, actions }: Props = $props();

	let dialogEl: HTMLDialogElement;

	$effect(() => {
		if (open && dialogEl) {
			dialogEl.showModal();
		} else if (dialogEl) {
			dialogEl.close();
		}
	});

	function handleClose() {
		open = false;
		onclose?.();
	}

	function handleBackdropClick(event: MouseEvent) {
		if (event.target === dialogEl) {
			handleClose();
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			handleClose();
		}
	}
</script>

<dialog
	bind:this={dialogEl}
	class="dialog"
	onclick={handleBackdropClick}
	onkeydown={handleKeyDown}
	oncancel={(e) => {
		e.preventDefault();
		handleClose();
	}}
>
	<div class="dialog-content">
		{#if title}
			<header class="dialog-header">
				<h2 class="dialog-title">{title}</h2>
				<button class="dialog-close" onclick={handleClose} aria-label="Close dialog">
					<svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M15 5L5 15M5 5l10 10" />
					</svg>
				</button>
			</header>
		{/if}

		<div class="dialog-body">
			{#if children}
				{@render children()}
			{/if}
		</div>

		{#if actions}
			<footer class="dialog-actions">
				{@render actions()}
			</footer>
		{/if}
	</div>
</dialog>

<style>
	.dialog {
		padding: 0;
		border: none;
		border-radius: 0.5rem;
		box-shadow:
			0 20px 25px -5px rgba(0, 0, 0, 0.1),
			0 8px 10px -6px rgba(0, 0, 0, 0.1);
		max-width: 32rem;
		width: calc(100% - 2rem);
	}

	.dialog::backdrop {
		background: rgba(0, 0, 0, 0.4);
	}

	.dialog-content {
		display: flex;
		flex-direction: column;
	}

	.dialog-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 1rem 1.5rem;
		border-bottom: 1px solid var(--color-border, #e2e8f0);
	}

	.dialog-title {
		margin: 0;
		font-size: 1.125rem;
		font-weight: 600;
		color: var(--color-text, #1e293b);
	}

	.dialog-close {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0.25rem;
		background: transparent;
		border: none;
		border-radius: 0.25rem;
		color: var(--color-text-muted, #64748b);
		cursor: pointer;
	}

	.dialog-close:hover {
		color: var(--color-text, #1e293b);
		background: var(--color-surface, #f1f5f9);
	}

	.dialog-close:focus-visible {
		outline: 2px solid var(--color-primary, #3b82f6);
		outline-offset: 2px;
	}

	.dialog-body {
		padding: 1.5rem;
	}

	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.75rem;
		padding: 1rem 1.5rem;
		border-top: 1px solid var(--color-border, #e2e8f0);
	}
</style>
