<script lang="ts">
	import type { SoundFile, QueryOptions, FolderScanProgress, FolderScanFile } from '$lib/types';
	import { db, viewer, folder, system, getAudioSrc, formatDuration, formatFileSize } from '$lib/api';
	import { Button, Input, LiveRegion, ProgressBar } from './index';
	import SoundList from './SoundList.svelte';
	import { onMount, onDestroy } from 'svelte';
	import type { UnlistenFn } from '@tauri-apps/api/event';

	type ViewMode = 'database' | 'folder' | 'empty';
	type SortBy = 'filename' | 'duration' | 'modifiedAt' | 'size';
	type SortOrder = 'asc' | 'desc';

	// Virtual scrolling constants
	const WINDOW_SIZE = 1000;

	let mode = $state<ViewMode>('empty');
	let dbPath = $state<string | null>(null);
	let folderPath = $state<string | null>(null);

	// Virtual scrolling state
	let files = $state<SoundFile[]>([]);          // Window of files (max WINDOW_SIZE)
	let windowOffset = $state(0);                  // First index in current window
	let virtualIndex = $state(0);                  // Selected index in full list
	let totalCount = $state(0);                    // Total files matching filters

	// Folder mode: store all files locally (no virtual scrolling needed for reasonable sizes)
	let allFolderFiles = $state<SoundFile[]>([]);

	let playingFile = $state<SoundFile | null>(null);
	let searchQuery = $state('');
	let sortBy = $state<SortBy>('filename');
	let sortOrder = $state<SortOrder>('asc');
	let minDuration = $state<number | null>(null);
	let maxDuration = $state<number | null>(null);
	let isPlaying = $state(false);
	let isScanning = $state(false);
	let scanProgress = $state<FolderScanProgress | null>(null);
	let statusMessage = $state('');
	let liveRegionPoliteness = $state<'polite' | 'assertive'>('polite');

	// HTMLAudioElement for streaming playback (doesn't load entire file into memory)
	let audioElement: HTMLAudioElement | null = null;

	let unsubscribeProgress: UnlistenFn | null = null;
	let unsubscribeComplete: UnlistenFn | null = null;

	// Debounce search/filter
	let searchTimeout: ReturnType<typeof setTimeout>;

	// Get current selected file from window
	const selectedFile = $derived(() => {
		const domIndex = virtualIndex - windowOffset;
		return domIndex >= 0 && domIndex < files.length ? files[domIndex] : null;
	});

	// Build query options from current filter state
	function buildQueryOptions(offset?: number, limit?: number): QueryOptions {
		return {
			search: searchQuery || undefined,
			minDurationMs: minDuration ?? undefined,
			maxDurationMs: maxDuration ?? undefined,
			sortBy,
			sortOrder,
			offset,
			limit
		};
	}

	// Reactive effect that triggers on filter/sort changes
	$effect(() => {
		// Read all filter values to establish dependencies
		const _search = searchQuery;
		const _sortBy = sortBy;
		const _sortOrder = sortOrder;
		const _minDuration = minDuration;
		const _maxDuration = maxDuration;
		const _mode = mode;

		clearTimeout(searchTimeout);
		searchTimeout = setTimeout(() => {
			if (_mode === 'database') {
				// Reset to beginning on filter change
				virtualIndex = 0;
				windowOffset = 0;
				loadDatabaseWindow(0);
			} else if (_mode === 'folder') {
				filterAndSortFolderFiles();
			}
		}, 300);
	});

	// Track previous file count for live region announcements
	let previousFileCount = $state(-1);

	$effect(() => {
		const count = totalCount;
		if (previousFileCount !== -1 && previousFileCount !== count) {
			statusMessage = `${count} sound${count === 1 ? '' : 's'} found`;
		}
		previousFileCount = count;
	});

	onMount(async () => {
		unsubscribeProgress = await folder.onProgress((p) => {
			scanProgress = p;
		});

		unsubscribeComplete = await folder.onComplete((scannedFiles) => {
			isScanning = false;
			scanProgress = null;
			allFolderFiles = scannedFiles.map((f: FolderScanFile, index: number) => ({
				id: index,
				directoryId: 0,
				relativePath: f.relativePath,
				filename: f.filename,
				fullPath: f.fullPath,
				extension: f.extension,
				fileSize: f.fileSize,
				modifiedAt: f.modifiedAt,
				durationMs: f.durationMs,
				sampleRate: f.sampleRate,
				channels: f.channels,
				bitRate: f.bitRate,
				codec: f.codec,
				title: f.title,
				artist: f.artist,
				album: f.album,
				genre: f.genre,
				comment: f.comment,
				indexedAt: Date.now()
			}));
			filterAndSortFolderFiles();
			liveRegionPoliteness = 'assertive';
			statusMessage = `Folder loaded: ${scannedFiles.length} sound files found`;
			setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
		});

		await tryLoadLastDatabase();
	});

	async function tryLoadLastDatabase() {
		const lastDb = localStorage.getItem('lastDbPath');
		if (!lastDb) {
			return;
		}

		try {
			await db.open(lastDb);
			dbPath = lastDb;
			mode = 'database';
			await loadDatabaseWindow(0);
			statusMessage = 'Database loaded';
		} catch (_e) {
			localStorage.removeItem('lastDbPath');
		}
	}

	onDestroy(() => {
		clearTimeout(searchTimeout);
		unsubscribeProgress?.();
		unsubscribeComplete?.();
		if (audioElement) {
			audioElement.pause();
			audioElement.src = '';
			audioElement = null;
		}
	});

	// Load a window of files from database
	async function loadDatabaseWindow(offset: number) {
		try {
			const options = buildQueryOptions(offset, WINDOW_SIZE);
			const [result, count] = await Promise.all([
				viewer.query(options),
				viewer.count(buildQueryOptions())
			]);

			if (result.length > 0 || dbPath) {
				mode = 'database';
				files = result;
				windowOffset = offset;
				totalCount = count;
			} else if (mode === 'empty') {
				files = [];
				totalCount = 0;
			}
		} catch (_e) {
			// Database not open
		}
	}

	// Navigate to a specific virtual index, loading new window if needed
	async function handleNavigate(newIndex: number) {
		if (mode === 'folder') {
			// Folder mode: simple navigation within loaded files
			virtualIndex = Math.max(0, Math.min(newIndex, totalCount - 1));
			return;
		}

		// Database mode: virtual scrolling
		newIndex = Math.max(0, Math.min(newIndex, totalCount - 1));
		virtualIndex = newIndex;

		// Check if new index is within current window
		const windowEnd = windowOffset + files.length;
		if (newIndex >= windowOffset && newIndex < windowEnd) {
			// Already in window, no fetch needed
			return;
		}

		// Need to load a new window centered on the new index
		// Position window so selected item is roughly in the middle
		let newOffset = Math.max(0, newIndex - Math.floor(WINDOW_SIZE / 2));
		// But don't go past the end
		if (newOffset + WINDOW_SIZE > totalCount) {
			newOffset = Math.max(0, totalCount - WINDOW_SIZE);
		}

		await loadDatabaseWindow(newOffset);
	}

	// Handle letter navigation - find first file starting with character
	async function handleLetterNavigate(char: string, fromIndex: number) {
		if (mode === 'folder') {
			// Folder mode: search locally
			const lowerChar = char.toLowerCase();
			// Search from current position + 1 to end
			for (let i = fromIndex + 1; i < files.length; i++) {
				if (files[i].filename.toLowerCase().startsWith(lowerChar)) {
					virtualIndex = i;
					return;
				}
			}
			// Wrap around: search from beginning
			for (let i = 0; i <= fromIndex; i++) {
				if (files[i].filename.toLowerCase().startsWith(lowerChar)) {
					virtualIndex = i;
					return;
				}
			}
			return;
		}

		// Database mode: use backend to find index
		if (sortBy !== 'filename') {
			// Letter navigation only works for filename sort
			statusMessage = 'Letter navigation only works when sorted by name';
			return;
		}

		try {
			const options = buildQueryOptions();
			const index = await viewer.findPrefixIndex(options, char);
			if (index !== null && index < totalCount) {
				await handleNavigate(index);
			}
		} catch (e) {
			console.error('Letter navigation error:', e);
		}
	}

	function filterAndSortFolderFiles() {
		let filtered = allFolderFiles;

		if (searchQuery.trim()) {
			const query = searchQuery.toLowerCase();
			filtered = filtered.filter(
				(f) =>
					f.filename.toLowerCase().includes(query) ||
					f.relativePath.toLowerCase().includes(query) ||
					f.title?.toLowerCase().includes(query) ||
					f.artist?.toLowerCase().includes(query)
			);
		}

		if (minDuration !== null) {
			const min = minDuration;
			filtered = filtered.filter((f) => f.durationMs !== null && f.durationMs >= min);
		}
		if (maxDuration !== null) {
			const max = maxDuration;
			filtered = filtered.filter((f) => f.durationMs !== null && f.durationMs <= max);
		}

		filtered = [...filtered].sort((a, b) => {
			let comparison = 0;
			switch (sortBy) {
				case 'filename':
					comparison = a.filename.localeCompare(b.filename);
					break;
				case 'duration':
					comparison = (a.durationMs ?? 0) - (b.durationMs ?? 0);
					break;
				case 'modifiedAt':
					comparison = a.modifiedAt - b.modifiedAt;
					break;
			}
			return sortOrder === 'asc' ? comparison : -comparison;
		});

		// For folder mode, keep all files in memory (no backend pagination)
		// This works well for reasonable folder sizes
		files = filtered;
		windowOffset = 0;
		virtualIndex = 0;
		totalCount = filtered.length;
	}

	async function handleOpenDatabase() {
		const path = await db.browse();
		if (path) {
			await db.open(path);
			dbPath = path;
			mode = 'database';
			folderPath = null;
			allFolderFiles = [];
			virtualIndex = 0;
			windowOffset = 0;
			localStorage.setItem('lastDbPath', path);
			await loadDatabaseWindow(0);
			liveRegionPoliteness = 'assertive';
			statusMessage = 'Database opened';
			setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
		}
	}

	async function handleOpenFolder() {
		const path = await folder.browse();
		if (path) {
			folderPath = path;
			mode = 'folder';
			isScanning = true;
			files = [];
			allFolderFiles = [];
			totalCount = 0;
			virtualIndex = 0;
			windowOffset = 0;
			statusMessage = 'Scanning folder...';
			await folder.scan(path);
		}
	}

	async function handlePlay(file: SoundFile) {
		// Toggle off if same file is playing
		if (isPlaying && playingFile?.id === file.id) {
			if (audioElement) {
				audioElement.pause();
				audioElement.currentTime = 0;
			}
			isPlaying = false;
			playingFile = null;
			statusMessage = 'Playback stopped';
			return;
		}

		// Stop any current playback
		if (audioElement) {
			audioElement.pause();
			audioElement.currentTime = 0;
		}

		isPlaying = true;
		playingFile = file;
		statusMessage = `Playing: ${file.filename}`;

		try {
			// Create audio element if needed (reuse for efficiency)
			if (!audioElement) {
				audioElement = new Audio();
				audioElement.addEventListener('ended', () => {
					isPlaying = false;
					playingFile = null;
				});
				audioElement.addEventListener('error', (e) => {
					console.error('Audio playback error:', e);
					isPlaying = false;
					playingFile = null;
					liveRegionPoliteness = 'assertive';
					statusMessage = `Error: Could not play file`;
					setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
				});
			}

			// Set source and play - HTMLAudioElement streams from disk
			audioElement.src = getAudioSrc(file.fullPath);
			await audioElement.play();
		} catch (err) {
			console.error('handlePlay: play error', err);
			isPlaying = false;
			playingFile = null;
			liveRegionPoliteness = 'assertive';
			statusMessage = `Error: Could not play ${file.filename}`;
			setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
		}
	}

	async function handleOpenInExplorer(file: SoundFile) {
		await system.openInExplorer(file.fullPath);
	}

	async function handleContextMenu(file: SoundFile, _event: MouseEvent) {
		await system.openInExplorer(file.fullPath);
	}

	async function copyPath() {
		const file = selectedFile();
		if (!file) return;
		await system.copyToClipboard(file.fullPath);
		statusMessage = 'Path copied to clipboard';
	}

	async function openInExplorer() {
		const file = selectedFile();
		if (!file) return;
		await system.openInExplorer(file.fullPath);
	}

	// F1 - Show info about current file
	function handleShowInfo() {
		const file = selectedFile();
		if (!file) return;
		const itemNum = virtualIndex + 1;
		const duration = formatDuration(file.durationMs);
		const size = formatFileSize(file.fileSize);
		liveRegionPoliteness = 'polite';
		statusMessage = `Item ${itemNum} of ${totalCount.toLocaleString()} - ${duration} - ${size}`;
	}

	// Shift+Home - Copy paths from current to start
	async function handleCopyPathsToStart() {
		if (totalCount === 0 || virtualIndex < 0) return;

		try {
			let paths: string[];
			if (mode === 'folder') {
				// Folder mode: get paths from local array (0 to virtualIndex inclusive)
				paths = files.slice(0, virtualIndex + 1).map(f => f.fullPath);
			} else {
				// Database mode: get paths from backend
				paths = await viewer.getPaths(buildQueryOptions(), 0, virtualIndex);
			}

			if (paths.length > 0) {
				await system.copyToClipboard(paths.join('\n'));
				liveRegionPoliteness = 'assertive';
				statusMessage = `Copied ${paths.length} path${paths.length === 1 ? '' : 's'} to clipboard`;
				setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
			}
		} catch (e) {
			console.error('Copy paths to start error:', e);
			liveRegionPoliteness = 'assertive';
			statusMessage = 'Error copying paths';
			setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
		}
	}

	// Shift+End - Copy paths from current to end
	async function handleCopyPathsToEnd() {
		if (totalCount === 0 || virtualIndex < 0) return;

		try {
			let paths: string[];
			if (mode === 'folder') {
				// Folder mode: get paths from local array (virtualIndex to end)
				paths = files.slice(virtualIndex).map(f => f.fullPath);
			} else {
				// Database mode: get paths from backend
				paths = await viewer.getPaths(buildQueryOptions(), virtualIndex, totalCount - 1);
			}

			if (paths.length > 0) {
				await system.copyToClipboard(paths.join('\n'));
				liveRegionPoliteness = 'assertive';
				statusMessage = `Copied ${paths.length} path${paths.length === 1 ? '' : 's'} to clipboard`;
				setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
			}
		} catch (e) {
			console.error('Copy paths to end error:', e);
			liveRegionPoliteness = 'assertive';
			statusMessage = 'Error copying paths';
			setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
		}
	}

	// Cmd+A - Copy all paths
	async function handleCopyAllPaths() {
		if (totalCount === 0) return;

		try {
			let paths: string[];
			if (mode === 'folder') {
				// Folder mode: get all paths from local array
				paths = files.map(f => f.fullPath);
			} else {
				// Database mode: get all paths from backend
				paths = await viewer.getPaths(buildQueryOptions());
			}

			if (paths.length > 0) {
				await system.copyToClipboard(paths.join('\n'));
				liveRegionPoliteness = 'assertive';
				statusMessage = `Copied ${paths.length} path${paths.length === 1 ? '' : 's'} to clipboard`;
				setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
			}
		} catch (e) {
			console.error('Copy all paths error:', e);
			liveRegionPoliteness = 'assertive';
			statusMessage = 'Error copying paths';
			setTimeout(() => { liveRegionPoliteness = 'polite'; }, 100);
		}
	}
</script>

<div class="viewer-view">
	<header class="view-header">
		<div class="header-left">
			<div class="search-container">
				<Input
					type="search"
					placeholder="Search sounds..."
					bind:value={searchQuery}
					disabled={mode === 'empty' || isScanning}
				/>
			</div>
			{#if mode !== 'empty'}
				<div class="mode-indicator text-xs text-slate-500 dark:text-slate-400">
					{#if mode === 'database'}
						<span class="font-mono truncate max-w-32" title={dbPath ?? ''}>
							DB: {dbPath?.split(/[/\\]/).pop()}
						</span>
					{:else if mode === 'folder'}
						<span class="font-mono truncate max-w-32" title={folderPath ?? ''}>
							Folder: {folderPath?.split(/[/\\]/).pop()}
						</span>
					{/if}
				</div>
			{/if}
		</div>

		<div class="header-right flex gap-2 items-center">
			<span class="result-count text-sm text-slate-500 dark:text-slate-400">
				{#if totalCount > WINDOW_SIZE && mode === 'database'}
					{virtualIndex + 1} of {totalCount.toLocaleString()} files
				{:else}
					{totalCount.toLocaleString()} files
				{/if}
			</span>
			<Button size="sm" variant="ghost" onclick={handleOpenFolder}>
				Open Folder
			</Button>
			<Button size="sm" variant="primary" onclick={handleOpenDatabase}>
				Open DB
			</Button>
		</div>
	</header>

	{#if mode !== 'empty'}
		<div class="filter-panel">
			<fieldset class="filter-fieldset">
				<legend>Sort by</legend>
				<div class="radio-group">
					<label class="radio-label">
						<input type="radio" name="sortBy" value="filename" bind:group={sortBy} />
						Name
					</label>
					<label class="radio-label">
						<input type="radio" name="sortBy" value="duration" bind:group={sortBy} />
						Duration
					</label>
					<label class="radio-label">
						<input type="radio" name="sortBy" value="modifiedAt" bind:group={sortBy} />
						Date
					</label>
					<label class="radio-label">
						<input type="radio" name="sortBy" value="size" bind:group={sortBy} />
						Size
					</label>
				</div>
				<div class="radio-group">
					<label class="radio-label">
						<input type="radio" name="sortOrder" value="asc" bind:group={sortOrder} />
						Ascending
					</label>
					<label class="radio-label">
						<input type="radio" name="sortOrder" value="desc" bind:group={sortOrder} />
						Descending
					</label>
				</div>
			</fieldset>

			<fieldset class="filter-fieldset">
				<legend>Duration range (ms)</legend>
				<div class="duration-inputs">
					<label class="duration-label">
						Min
						<input type="number" class="duration-input" bind:value={minDuration} min="0" step="1" />
					</label>
					<span class="duration-separator">to</span>
					<label class="duration-label">
						Max
						<input type="number" class="duration-input" bind:value={maxDuration} min="0" step="1" />
					</label>
				</div>
			</fieldset>
		</div>
	{/if}

	{#if isScanning}
		<div class="scanning-progress">
			<ProgressBar
				value={scanProgress?.scanned ?? 0}
				max={100}
				label="Scanning..."
				showPercent={false}
			/>
			<div class="text-sm text-slate-500 dark:text-slate-400 mt-1">
				{scanProgress?.currentFile ?? 'Scanning...'}
			</div>
		</div>
	{/if}

	{#if mode === 'empty' && !isScanning}
		<div class="empty-state">
			<div class="empty-content">
				<p class="text-slate-500 dark:text-slate-400">
					Open a folder or database to browse sounds.
				</p>
			</div>
		</div>
	{:else}
		<SoundList
			{files}
			{windowOffset}
			{totalCount}
			{virtualIndex}
			onNavigate={handleNavigate}
			onPlay={handlePlay}
			onOpenInExplorer={handleOpenInExplorer}
			onContextMenu={handleContextMenu}
			onLetterNavigate={handleLetterNavigate}
			onShowInfo={handleShowInfo}
			onCopyPathsToStart={handleCopyPathsToStart}
			onCopyPathsToEnd={handleCopyPathsToEnd}
			onCopyAllPaths={handleCopyAllPaths}
		/>
	{/if}

	{#if selectedFile()}
		<footer class="view-footer">
			<div class="selected-info">
				<span class="font-medium">{selectedFile()?.filename}</span>
				{#if isPlaying}
					<span class="playing-indicator">Playing...</span>
				{/if}
			</div>
			<div class="actions flex gap-2">
				<Button size="sm" variant="ghost" onclick={() => handlePlay(selectedFile()!)}>
					{isPlaying ? 'Stop' : 'Play'}
				</Button>
				<Button size="sm" variant="ghost" onclick={copyPath}>
					Copy Path
				</Button>
				<Button size="sm" variant="ghost" onclick={openInExplorer}>
					Show in Explorer
				</Button>
			</div>
		</footer>
	{/if}

	<LiveRegion message={statusMessage} politeness={liveRegionPoliteness} />
</div>

<style>
	.viewer-view {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.view-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--color-border);
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		flex: 1;
	}

	.search-container {
		flex: 1;
		max-width: 20rem;
	}

	.mode-indicator {
		display: flex;
		align-items: center;
	}

	.header-right {
		flex-shrink: 0;
	}

	.filter-panel {
		display: flex;
		flex-wrap: wrap;
		gap: 1.5rem;
		padding: 0.75rem 1rem;
		background: var(--color-surface);
		border-bottom: 1px solid var(--color-border);
	}

	.filter-fieldset {
		border: 1px solid var(--color-border);
		border-radius: 0.375rem;
		padding: 0.5rem 0.75rem;
		margin: 0;
	}

	.filter-fieldset legend {
		font-size: 0.875rem;
		font-weight: 500;
		padding: 0 0.25rem;
		color: var(--color-text);
	}

	.radio-group {
		display: flex;
		gap: 1rem;
		margin-top: 0.25rem;
	}

	.radio-label {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		font-size: 0.875rem;
		cursor: pointer;
	}

	.radio-label input[type='radio'] {
		margin: 0;
		cursor: pointer;
	}

	.duration-inputs {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.25rem;
	}

	.duration-label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		font-size: 0.75rem;
		color: var(--color-text-muted);
	}

	.duration-separator {
		font-size: 0.875rem;
		color: var(--color-text-muted);
	}

	.duration-input {
		width: 5rem;
		padding: 0.25rem 0.5rem;
		font-size: 0.875rem;
		border: 1px solid var(--color-border);
		border-radius: 0.25rem;
		background: var(--color-background);
		color: var(--color-text);
	}

	.duration-input:focus {
		outline: none;
		border-color: var(--color-primary);
	}

	.scanning-progress {
		padding: 1rem;
		background: var(--color-surface);
		border-bottom: 1px solid var(--color-border);
	}

	.empty-state {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.empty-content {
		text-align: center;
		max-width: 28rem;
	}

	.view-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		border-top: 1px solid var(--color-border);
		background: var(--color-surface);
	}

	.selected-info {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.playing-indicator {
		color: var(--color-primary);
		font-size: 0.875rem;
	}
</style>
