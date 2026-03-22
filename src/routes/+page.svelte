<script lang="ts">
	import { Tabs, IndexingView, McpView, ViewerView } from '$lib/components';

	const tabs = [
		{ id: 'viewer', label: 'Viewer' },
		{ id: 'indexing', label: 'Indexing' },
		{ id: 'mcp', label: 'MCP' }
	];

	let activeTab = $state('viewer');
</script>

<div class="app">
	<header class="app-header">
		<h1 class="app-title">Sound Effect Indexer</h1>
		<nav>
			<Tabs {tabs} bind:activeTab />
		</nav>
		<div class="spacer"></div>
	</header>

	<main class="app-content">
		{#if activeTab === 'viewer'}
			<ViewerView />
		{:else if activeTab === 'indexing'}
			<IndexingView />
		{:else if activeTab === 'mcp'}
			<McpView />
		{/if}
	</main>
</div>

<style>
	.app {
		display: flex;
		flex-direction: column;
		height: 100vh;
	}

	.app-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 3.5rem;
		padding: 0 1rem;
		background: var(--color-surface);
		border-bottom: 1px solid var(--color-border);
		-webkit-app-region: drag;
	}

	.app-header nav {
		-webkit-app-region: no-drag;
	}

	.app-title {
		font-size: 1rem;
		font-weight: 600;
	}

	.spacer {
		width: 8rem;
	}

	.app-content {
		flex: 1;
		overflow: hidden;
	}
</style>
