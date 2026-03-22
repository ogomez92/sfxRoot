<script lang="ts">
	import type { SoundFile } from '$lib/types';
	import { formatDuration, formatFileSize } from '$lib/api';

	interface Props {
		files: SoundFile[];              // Window of files currently loaded (max 1000)
		windowOffset: number;            // Index of first file in the window within full list
		totalCount: number;              // Total files in full list
		virtualIndex: number;            // Currently selected index in full list
		onNavigate: (index: number) => void;  // Called when navigation changes virtual index
		onPlay: (file: SoundFile) => void;
		onOpenInExplorer: (file: SoundFile) => void;
		onContextMenu: (file: SoundFile, event: MouseEvent) => void;
		onLetterNavigate: (char: string, fromIndex: number) => void;  // For letter navigation
		onShowInfo: () => void;          // F1 - show current file info
		onCopyPathsToStart: () => void;  // Shift+Home - copy paths from current to start
		onCopyPathsToEnd: () => void;    // Shift+End - copy paths from current to end
		onCopyAllPaths: () => void;      // Cmd+A - copy all paths
	}

	let {
		files,
		windowOffset,
		totalCount,
		virtualIndex,
		onNavigate,
		onPlay,
		onOpenInExplorer,
		onContextMenu,
		onLetterNavigate,
		onShowInfo,
		onCopyPathsToStart,
		onCopyPathsToEnd,
		onCopyAllPaths
	}: Props = $props();

	let listEl: HTMLElement;
	let hasInitiallyFocused = false;

	// Calculate DOM index from virtual index
	const domIndex = $derived(virtualIndex - windowOffset);
	const selectedFile = $derived(domIndex >= 0 && domIndex < files.length ? files[domIndex] : null);

	// Auto-focus list only on initial load
	$effect(() => {
		if (files.length > 0 && listEl && !hasInitiallyFocused) {
			hasInitiallyFocused = true;
			listEl.focus();
		}
		if (files.length === 0) {
			hasInitiallyFocused = false;
		}
	});

	// Scroll selected item into view when domIndex changes
	$effect(() => {
		if (selectedFile && domIndex >= 0) {
			requestAnimationFrame(() => {
				const element = document.getElementById(`sound-${selectedFile.id}`);
				element?.scrollIntoView({ block: 'nearest' });
			});
		}
	});

	function handleKeyDown(event: KeyboardEvent) {
		if (totalCount === 0) return;

		// Page size is 10% of total list, minimum 1, max 100
		const pageSize = Math.max(1, Math.min(100, Math.floor(totalCount * 0.1)));

		// Handle F1 - show info
		if (event.key === 'F1') {
			event.preventDefault();
			onShowInfo();
			listEl?.focus();
			return;
		}

		// Handle Shift+Home - copy paths to start
		if (event.key === 'Home' && event.shiftKey) {
			event.preventDefault();
			onCopyPathsToStart();
			listEl?.focus();
			return;
		}

		// Handle Shift+End - copy paths to end
		if (event.key === 'End' && event.shiftKey) {
			event.preventDefault();
			onCopyPathsToEnd();
			listEl?.focus();
			return;
		}

		// Handle Cmd+A (Mac) or Ctrl+A (Windows/Linux) - copy all paths
		if (event.key === 'a' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			onCopyAllPaths();
			listEl?.focus();
			return;
		}

		switch (event.key) {
			case 'ArrowDown':
				event.preventDefault();
				if (virtualIndex < totalCount - 1) {
					onNavigate(virtualIndex + 1);
				}
				break;
			case 'ArrowUp':
				event.preventDefault();
				if (virtualIndex > 0) {
					onNavigate(virtualIndex - 1);
				}
				break;
			case 'PageDown':
				event.preventDefault();
				onNavigate(Math.min(virtualIndex + pageSize, totalCount - 1));
				break;
			case 'PageUp':
				event.preventDefault();
				onNavigate(Math.max(virtualIndex - pageSize, 0));
				break;
			case 'Home':
				event.preventDefault();
				onNavigate(0);
				break;
			case 'End':
				event.preventDefault();
				onNavigate(totalCount - 1);
				break;
			case ' ':
				event.preventDefault();
				if (selectedFile) {
					onPlay(selectedFile);
				}
				break;
			case 'Enter':
				event.preventDefault();
				if (selectedFile) {
					onOpenInExplorer(selectedFile);
				}
				break;
			default:
				// Type-ahead: jump to next file starting with pressed character
				if (event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey) {
					event.preventDefault();
					onLetterNavigate(event.key, virtualIndex);
				}
				break;
		}

		// Keep focus on list
		listEl?.focus();
	}

	function handleClick(file: SoundFile, index: number) {
		onNavigate(windowOffset + index);
	}
</script>

<div
	class="sound-list"
	role="listbox"
	tabindex="0"
	bind:this={listEl}
	onkeydown={handleKeyDown}
	aria-label="Sound files"
	aria-activedescendant={selectedFile ? `sound-${selectedFile.id}` : undefined}
>
	{#if files.length === 0}
		<div class="empty-state">
			<p class="text-slate-500 dark:text-slate-400">No sound files found</p>
		</div>
	{:else}
		{#each files as file, index (file.id)}
			{@const isSelected = index === domIndex}
			<div
				id="sound-{file.id}"
				class="sound-item"
				class:selected={isSelected}
				role="option"
				tabindex="-1"
				aria-selected={isSelected}
				aria-label="{file.filename}, {formatDuration(file.durationMs)}, {formatFileSize(file.fileSize)}"
				onclick={() => handleClick(file, index)}
				ondblclick={() => onPlay(file)}
				onkeydown={(e) => {
					if (e.key === 'Enter') onOpenInExplorer(file);
					else if (e.key === ' ') {
						e.preventDefault();
						onPlay(file);
					}
				}}
				oncontextmenu={(e) => {
					e.preventDefault();
					onContextMenu(file, e);
				}}
			>
				<div class="sound-info">
					<div class="sound-name">{file.filename}</div>
					<div class="sound-meta text-xs text-slate-500 dark:text-slate-400">
						{file.relativePath}
					</div>
				</div>
				<div class="sound-duration font-mono text-sm text-slate-600 dark:text-slate-300">
					{formatDuration(file.durationMs)}
				</div>
				<div class="sound-size text-xs text-slate-500 dark:text-slate-400">
					{formatFileSize(file.fileSize)}
				</div>
			</div>
		{/each}
	{/if}
</div>

<style>
	.sound-list {
		flex: 1;
		overflow-y: auto;
		outline: none;
	}

	.sound-list:focus-visible {
		box-shadow: inset 0 0 0 2px var(--color-primary);
	}

	.empty-state {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		padding: 2rem;
	}

	.sound-item {
		display: grid;
		grid-template-columns: 1fr auto auto;
		gap: 1rem;
		align-items: center;
		padding: 0.5rem 1rem;
		cursor: pointer;
		border-bottom: 1px solid var(--color-border);
		outline: none;
	}

	.sound-item:hover {
		background: var(--color-surface);
	}

	.sound-item.selected {
		background: var(--color-primary);
		color: white;
	}

	.sound-item.selected .sound-meta,
	.sound-item.selected .sound-duration,
	.sound-item.selected .sound-size {
		color: rgba(255, 255, 255, 0.8);
	}

	.sound-info {
		min-width: 0;
	}

	.sound-name {
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sound-meta {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
