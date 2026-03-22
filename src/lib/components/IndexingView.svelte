<script lang="ts">
	import type { Directory, IndexingProgress } from '$lib/types';
	import { db, directories, indexing } from '$lib/api';
	import { Button, ProgressBar, Dialog, LiveRegion } from './index';
	import DirectoryList from './DirectoryList.svelte';
	import { onMount, onDestroy } from 'svelte';
	import type { UnlistenFn } from '@tauri-apps/api/event';

	// Helper to extract error message from unknown error type
	function getErrorMessage(e: unknown): string {
		if (typeof e === 'string') return e;
		if (e instanceof Error) return e.message;
		return String(e);
	}

	let directoriesList = $state<Directory[]>([]);
	let isIndexing = $state(false);
	let progress = $state<IndexingProgress | null>(null);
	let statusMessage = $state('');
	let statusPoliteness = $state<'polite' | 'assertive'>('polite');
	let dbPath = $state<string | null>(null);
	let showDbDialog = $state(false);

	let unsubscribeProgress: UnlistenFn | null = null;
	let unsubscribeComplete: UnlistenFn | null = null;

	// Track last announced percentage to avoid too frequent announcements
	let lastAnnouncedPercent = -1;

	onMount(async () => {
		// Subscribe to indexing events
		unsubscribeProgress = await indexing.onProgress((p) => {
			const prevPhase = progress?.phase;
			progress = p;

			// Phase labels for announcements
			const phaseLabels: Record<string, string> = {
				discovering: 'Discovering files',
				preparing: 'Reading file info',
				comparing: 'Comparing files',
				extracting: 'Extracting metadata',
				saving: 'Saving to database'
			};

			// Announce phase changes
			if (prevPhase !== p.phase) {
				lastAnnouncedPercent = -1; // Reset percentage tracking on phase change
				statusMessage = phaseLabels[p.phase] || 'Processing';
				return;
			}

			// For discovering phase (no total), just show count
			if (p.phase === 'discovering') {
				if (p.current > 0 && p.current % 1000 === 0) {
					statusMessage = `Found ${p.current.toLocaleString()} audio files...`;
				}
				return;
			}

			// Calculate percentage for phases with known total
			const percent = p.total > 0 ? Math.round((p.current / p.total) * 100) : 0;

			// Announce every 5% or every 1000 files (whichever is more frequent)
			const percentInterval = 5;
			const fileInterval = Math.min(1000, Math.max(1, Math.floor(p.total / 20)));
			const shouldAnnounce =
				lastAnnouncedPercent === -1 || // First update
				percent >= lastAnnouncedPercent + percentInterval || // Every 5%
				p.current % fileInterval === 0 || // Every ~1000 files
				percent === 100; // Completion

			if (shouldAnnounce) {
				lastAnnouncedPercent = percent;
				const phaseLabel = phaseLabels[p.phase] || 'Processing';
				statusMessage = `${phaseLabel}: ${percent}% (${p.current.toLocaleString()} of ${p.total.toLocaleString()})`;
			}
		});

		unsubscribeComplete = await indexing.onComplete(() => {
			isIndexing = false;
			progress = null;
			lastAnnouncedPercent = -1;
			statusMessage = 'Indexing complete';
			loadDirectories();
		});

		// Try to load last used database from localStorage
		await tryLoadLastDatabase();
	});

	async function tryLoadLastDatabase() {
		const lastDb = localStorage.getItem('lastDbPath');
		if (!lastDb) return;

		try {
			await db.open(lastDb);
			dbPath = lastDb;
			await loadDirectories();
		} catch (_e) {
			// DB file doesn't exist anymore - clear storage
			localStorage.removeItem('lastDbPath');
		}
	}

	onDestroy(() => {
		unsubscribeProgress?.();
		unsubscribeComplete?.();
	});

	async function loadDirectories() {
		try {
			directoriesList = await directories.list();
		} catch (e) {
			console.error('Load directories error:', e);
			statusMessage = `Error loading directories: ${getErrorMessage(e)}`;
		}
	}

	async function handleCreateDb() {
		const path = await db.create();
		if (path) {
			dbPath = path;
			showDbDialog = false;
			// Save to localStorage for next launch
			localStorage.setItem('lastDbPath', path);
			await loadDirectories();
			statusMessage = 'Database created';
		}
	}

	async function handleOpenDb() {
		const path = await db.browse();
		if (path) {
			await db.open(path);
			dbPath = path;
			showDbDialog = false;
			// Save to localStorage for next launch
			localStorage.setItem('lastDbPath', path);
			await loadDirectories();
			statusMessage = 'Database opened';
		}
	}

	async function handleAddDirectory() {
		// Check if DB is open
		if (!dbPath) {
			showDbDialog = true;
			return;
		}

		const path = await directories.browse();
		if (path) {
			isIndexing = true;
			lastAnnouncedPercent = -1;
			statusMessage = 'Starting indexing...';
			try {
				await indexing.start(path);
			} catch (e) {
				console.error('Indexing error:', e);
				statusPoliteness = 'assertive';
				statusMessage = `Indexing error: ${getErrorMessage(e)}`;
				setTimeout(() => { statusPoliteness = 'polite'; }, 100);
				isIndexing = false;
			}
		}
	}

	async function handleRemoveDirectory(id: number) {
		try {
			await directories.remove(id);
			await loadDirectories();
			statusMessage = 'Directory removed';
		} catch (e) {
			console.error('Remove directory error:', e);
			statusPoliteness = 'assertive';
			statusMessage = `Error removing directory: ${getErrorMessage(e)}`;
			setTimeout(() => { statusPoliteness = 'polite'; }, 100);
		}
	}

	async function handleResyncDirectory(id: number) {
		isIndexing = true;
		lastAnnouncedPercent = -1;
		statusMessage = 'Starting resync...';
		try {
			await indexing.resync(id);
		} catch (e) {
			console.error('Resync directory error:', e);
			statusPoliteness = 'assertive';
			statusMessage = `Error resyncing: ${getErrorMessage(e)}`;
			setTimeout(() => { statusPoliteness = 'polite'; }, 100);
			isIndexing = false;
		}
	}

	async function handleResyncAll() {
		if (directoriesList.length === 0) return;
		isIndexing = true;
		lastAnnouncedPercent = -1;
		statusMessage = 'Resyncing all directories...';

		for (let i = 0; i < directoriesList.length; i++) {
			const dir = directoriesList[i];
			lastAnnouncedPercent = -1; // Reset for each directory
			try {
				statusMessage = `Resyncing directory ${i + 1} of ${directoriesList.length}: ${dir.path.split(/[/\\]/).pop()}`;
				await indexing.resync(dir.id);
			} catch (e) {
				console.error('Resync all error:', e);
				statusPoliteness = 'assertive';
				statusMessage = `Error resyncing ${dir.path.split(/[/\\]/).pop()}: ${getErrorMessage(e)}`;
				setTimeout(() => { statusPoliteness = 'polite'; }, 100);
				// Continue with next directory
			}
		}

		isIndexing = false;
		lastAnnouncedPercent = -1;
		await loadDirectories();
		statusMessage = 'Resync complete';
	}

	async function handleCancelIndexing() {
		await indexing.cancel();
		isIndexing = false;
		statusMessage = 'Indexing cancelled';
	}
</script>

<div class="indexing-view">
	<header class="view-header">
		<h2 class="text-xl font-semibold">Manage Directories</h2>
		<div class="flex gap-2">
			{#if dbPath}
				<span class="text-sm text-slate-500 dark:text-slate-400 font-mono truncate max-w-xs">
					{dbPath}
				</span>
			{/if}
			<Button onclick={() => (showDbDialog = true)} variant="ghost" size="sm">
				{dbPath ? 'Change DB' : 'Open/Create DB'}
			</Button>
			<Button onclick={handleAddDirectory} variant="primary" disabled={isIndexing}>
				Add Directory
			</Button>
		</div>
	</header>

	{#if isIndexing}
		{@const phaseLabels: Record<string, string> = {
			discovering: 'Discovering files...',
			scanning: 'Scanning files...',
			comparing: 'Comparing files...',
			extracting: 'Extracting metadata...',
			saving: 'Saving to database...'
		}}
		{@const isDiscovering = !progress || progress.phase === 'discovering'}
		{@const hasTotalKnown = progress !== null && progress.total > 0}
		{@const percent = progress !== null && progress.total > 0 ? Math.round((progress.current / progress.total) * 100) : 0}
		<div class="indexing-progress">
			<div class="progress-header">
				<span class="progress-label">{progress ? (phaseLabels[progress.phase] || 'Processing...') : 'Starting...'}</span>
				{#if hasTotalKnown}
					<span class="progress-percent">{percent}%</span>
				{/if}
			</div>
			{#if isDiscovering}
				<div class="progress-bar-indeterminate"></div>
				<div class="progress-details text-sm text-slate-500 dark:text-slate-400 mt-2">
					{#if progress && progress.current > 0}
						Found {progress.current.toLocaleString()} audio files...
						{#if progress.currentFile}
							<span class="font-mono block truncate">{progress.currentFile}</span>
						{/if}
					{:else}
						Scanning directories...
					{/if}
				</div>
			{:else if progress}
				<ProgressBar value={progress.current} max={progress.total} label={phaseLabels[progress.phase]} showPercent={false} />
				<div class="progress-details text-sm text-slate-500 dark:text-slate-400 mt-2">
					{progress.current.toLocaleString()} / {progress.total.toLocaleString()} files
					{#if progress.currentFile}
						<span class="font-mono block truncate">{progress.currentFile}</span>
					{/if}
				</div>
			{/if}
			{#if progress?.stats}
				<div class="resync-stats text-sm mt-2">
					<span class="stat unchanged">{progress.stats.unchanged} unchanged</span>
					<span class="stat modified">{progress.stats.modified} modified</span>
					<span class="stat added">{progress.stats.added} new</span>
					<span class="stat deleted">{progress.stats.deleted} deleted</span>
				</div>
			{/if}
			<Button onclick={handleCancelIndexing} variant="ghost" size="sm">Cancel</Button>
		</div>
	{/if}

	<DirectoryList
		directories={directoriesList}
		onRemove={handleRemoveDirectory}
		onResync={handleResyncDirectory}
		onResyncAll={handleResyncAll}
		resyncDisabled={isIndexing}
	/>

	<LiveRegion message={statusMessage} politeness={statusPoliteness} />
</div>

<Dialog bind:open={showDbDialog} title="Database">
	{#snippet children()}
		<p class="mb-4 text-slate-600 dark:text-slate-300">
			Choose an existing database or create a new one to store your sound file index.
		</p>
	{/snippet}

	{#snippet actions()}
		<Button onclick={handleOpenDb} variant="secondary">Open Existing</Button>
		<Button onclick={handleCreateDb} variant="primary">Create New</Button>
	{/snippet}
</Dialog>

<style>
	.indexing-view {
		display: flex;
		flex-direction: column;
		height: 100%;
		padding: 1.5rem;
	}

	.view-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 1.5rem;
	}

	.indexing-progress {
		padding: 1rem;
		margin-bottom: 1rem;
		background: var(--color-surface);
		border: 1px solid var(--color-border);
		border-radius: 0.5rem;
	}

	.progress-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 0.5rem;
	}

	.progress-label {
		font-weight: 500;
	}

	.progress-percent {
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--color-primary);
	}

	.progress-bar-indeterminate {
		height: 0.5rem;
		background: var(--color-surface, #e2e8f0);
		border-radius: 9999px;
		overflow: hidden;
		position: relative;
	}

	.progress-bar-indeterminate::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		height: 100%;
		width: 30%;
		background: var(--color-primary, #3b82f6);
		border-radius: 9999px;
		animation: indeterminate 1.5s ease-in-out infinite;
	}

	@keyframes indeterminate {
		0% {
			left: -30%;
		}
		100% {
			left: 100%;
		}
	}

	.resync-stats {
		display: flex;
		gap: 1rem;
		flex-wrap: wrap;
	}

	.resync-stats .stat {
		padding: 0.125rem 0.5rem;
		border-radius: 0.25rem;
		font-weight: 500;
	}

	.resync-stats .unchanged {
		background: #e2e8f0;
		color: #475569;
	}

	.resync-stats .modified {
		background: #fef3c7;
		color: #92400e;
	}

	.resync-stats .added {
		background: #d1fae5;
		color: #065f46;
	}

	.resync-stats .deleted {
		background: #fee2e2;
		color: #991b1b;
	}

	:global(.dark) .resync-stats .unchanged {
		background: #334155;
		color: #94a3b8;
	}

	:global(.dark) .resync-stats .modified {
		background: #451a03;
		color: #fbbf24;
	}

	:global(.dark) .resync-stats .added {
		background: #064e3b;
		color: #6ee7b7;
	}

	:global(.dark) .resync-stats .deleted {
		background: #450a0a;
		color: #fca5a5;
	}
</style>
