<script lang="ts">
	interface Tab {
		id: string;
		label: string;
	}

	interface Props {
		tabs: Tab[];
		activeTab?: string;
		onchange?: (tabId: string) => void;
	}

	let { tabs, activeTab = $bindable(tabs[0]?.id ?? ''), onchange }: Props = $props();

	function handleClick(tabId: string) {
		activeTab = tabId;
		onchange?.(tabId);
	}

	function handleKeyDown(event: KeyboardEvent, index: number) {
		let newIndex = index;

		if (event.key === 'ArrowLeft') {
			newIndex = index === 0 ? tabs.length - 1 : index - 1;
		} else if (event.key === 'ArrowRight') {
			newIndex = index === tabs.length - 1 ? 0 : index + 1;
		} else if (event.key === 'Home') {
			newIndex = 0;
		} else if (event.key === 'End') {
			newIndex = tabs.length - 1;
		} else {
			return;
		}

		event.preventDefault();
		const button = event.currentTarget as HTMLElement;
		const parent = button.parentElement;
		const newButton = parent?.children[newIndex] as HTMLButtonElement;
		newButton?.focus();
		handleClick(tabs[newIndex].id);
	}
</script>

<div class="tabs" role="tablist">
	{#each tabs as tab, index (tab.id)}
		<button
			role="tab"
			aria-selected={activeTab === tab.id}
			tabindex={activeTab === tab.id ? 0 : -1}
			class="tab"
			class:active={activeTab === tab.id}
			onclick={() => handleClick(tab.id)}
			onkeydown={(e) => handleKeyDown(e, index)}
		>
			{tab.label}
		</button>
	{/each}
</div>

<style>
	.tabs {
		display: flex;
		gap: 0.25rem;
		border-bottom: 1px solid var(--color-border, #e2e8f0);
	}

	.tab {
		padding: 0.75rem 1rem;
		font-family: inherit;
		font-size: 0.9375rem;
		font-weight: 500;
		color: var(--color-text-muted, #64748b);
		background: transparent;
		border: none;
		border-bottom: 2px solid transparent;
		cursor: pointer;
		transition: all 0.15s ease;
		margin-bottom: -1px;
	}

	.tab:hover {
		color: var(--color-text, #1e293b);
	}

	.tab:focus-visible {
		outline: 2px solid var(--color-primary, #3b82f6);
		outline-offset: -2px;
	}

	.tab.active {
		color: var(--color-primary, #3b82f6);
		border-bottom-color: var(--color-primary, #3b82f6);
	}
</style>
