<script lang="ts">
	import type { Directory } from '$lib/types';
	import { Button } from './index';

	interface Props {
		directories: Directory[];
		onRemove: (id: number) => void;
		onResync: (id: number) => void;
		onResyncAll: () => void;
		resyncDisabled?: boolean;
	}

	let { directories, onRemove, onResync, onResyncAll, resyncDisabled = false }: Props = $props();

	function formatDate(timestamp: number | null): string {
		if (!timestamp) return 'Never';
		return new Date(timestamp * 1000).toLocaleString();
	}

	function getDirectoryName(path: string): string {
		return path.split(/[/\\]/).pop() || path;
	}
</script>

<div class="directory-list">
	{#if directories.length === 0}
		<div class="empty-state">
			<p class="text-slate-500 dark:text-slate-400">
				No directories indexed yet. Add a directory to get started.
			</p>
		</div>
	{:else}
		<div class="table-actions">
			<Button size="sm" variant="secondary" onclick={onResyncAll} disabled={resyncDisabled}>
				Resync All
			</Button>
		</div>
		<table class="directory-table">
			<thead>
				<tr>
					<th scope="col">Name</th>
					<th scope="col">Files</th>
					<th scope="col">Last Sync</th>
					<th scope="col">Actions</th>
				</tr>
			</thead>
			<tbody>
				{#each directories as dir (dir.id)}
					<tr>
						<td>
							<span class="directory-name" title={dir.path}>
								{getDirectoryName(dir.path)}
							</span>
						</td>
						<td class="text-center">{dir.fileCount}</td>
						<td>{formatDate(dir.lastSyncedAt)}</td>
						<td class="actions-cell">
							<Button size="sm" variant="ghost" onclick={() => onResync(dir.id)} disabled={resyncDisabled}>
								Resync
							</Button>
							<Button size="sm" variant="danger" onclick={() => onRemove(dir.id)}>
								Remove
							</Button>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</div>

<style>
	.directory-list {
		width: 100%;
	}

	.empty-state {
		padding: 2rem;
		text-align: center;
	}

	.table-actions {
		margin-bottom: 1rem;
	}

	.directory-table {
		width: 100%;
		border-collapse: collapse;
	}

	.directory-table th,
	.directory-table td {
		padding: 0.75rem 1rem;
		text-align: left;
		border-bottom: 1px solid var(--color-border);
	}

	.directory-table th {
		font-weight: 600;
		font-size: 0.875rem;
		color: var(--color-text-muted);
		background: var(--color-surface);
	}

	.directory-table tbody tr:hover {
		background: var(--color-surface);
	}

	.directory-name {
		font-family: monospace;
		font-size: 0.875rem;
	}

	.actions-cell {
		display: flex;
		gap: 0.5rem;
	}
</style>
