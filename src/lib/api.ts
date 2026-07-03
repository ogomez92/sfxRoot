// Tauri IPC API wrapper
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/core';
import type {
	Directory,
	IncompleteDirectory,
	SoundFile,
	QueryOptions,
	IndexingProgress,
	IndexingResult,
	FolderScanFile,
	FolderScanProgress
} from './types';

// Database commands
export const db = {
	browse: (): Promise<string | null> => invoke('db_browse'),
	create: (): Promise<string | null> => invoke('db_create'),
	open: (path: string): Promise<void> => invoke('db_open', { path }),
	close: (): Promise<void> => invoke('db_close'),
	isOpen: (): Promise<boolean> => invoke('db_is_open')
};

// Directory commands
export const directories = {
	browse: (): Promise<string | null> => invoke('directories_browse'),
	list: (): Promise<Directory[]> => invoke('directories_list'),
	add: (path: string): Promise<Directory> => invoke('directories_add', { path }),
	remove: (id: number): Promise<void> => invoke('directories_remove', { id }),
	get: (id: number): Promise<Directory | null> => invoke('directories_get', { id }),
	incomplete: (): Promise<IncompleteDirectory[]> => invoke('directories_incomplete')
};

// Indexing commands
export const indexing = {
	start: (path: string): Promise<IndexingResult> => invoke('indexing_start', { path }),
	resync: (directoryId: number): Promise<IndexingResult> =>
		invoke('indexing_resync', { directoryId }),
	resume: (directoryId: number): Promise<IndexingResult> =>
		invoke('indexing_resume', { directoryId }),
	cancel: (): Promise<void> => invoke('indexing_cancel'),

	// Event listeners
	onProgress: (callback: (progress: IndexingProgress) => void): Promise<UnlistenFn> =>
		listen<IndexingProgress>('indexing:progress', (event) => callback(event.payload)),

	onComplete: (callback: (result: IndexingResult) => void): Promise<UnlistenFn> =>
		listen<IndexingResult>('indexing:complete', (event) => callback(event.payload))
};

// Viewer/Search commands
export const viewer = {
	query: (options: QueryOptions): Promise<SoundFile[]> => invoke('viewer_query', { options }),
	count: (options: QueryOptions): Promise<number> => invoke('viewer_count', { options }),
	findPrefixIndex: (options: QueryOptions, prefix: string): Promise<number | null> =>
		invoke('viewer_find_prefix_index', { options, prefix }),
	getPaths: (options: QueryOptions, fromIndex?: number, toIndex?: number): Promise<string[]> =>
		invoke('viewer_get_paths', { options, fromIndex, toIndex })
};

// Folder scanning (without database)
export const folder = {
	browse: (): Promise<string | null> => invoke('folder_browse'),
	scan: (path: string): Promise<void> => invoke('folder_scan', { path }),

	// Event listeners
	onProgress: (callback: (progress: FolderScanProgress) => void): Promise<UnlistenFn> =>
		listen<FolderScanProgress>('folder:progress', (event) => callback(event.payload)),

	onComplete: (callback: (files: FolderScanFile[]) => void): Promise<UnlistenFn> =>
		listen<FolderScanFile[]>('folder:complete', (event) => callback(event.payload))
};

// MCP server commands
export interface McpStatus {
	running: boolean;
	port: number;
	dbPath: string | null;
}

export const mcp = {
	start: (port: number): Promise<McpStatus> => invoke('mcp_start', { port }),
	stop: (): Promise<void> => invoke('mcp_stop'),
	status: (): Promise<McpStatus> => invoke('mcp_status'),
	getDbPath: (): Promise<string | null> => invoke('mcp_get_db_path'),
	saveSkill: (): Promise<string | null> => invoke('mcp_save_skill')
};

// System commands
export const system = {
	openInExplorer: (path: string): Promise<void> => invoke('open_in_explorer', { path }),
	copyToClipboard: (text: string): Promise<void> => invoke('copy_to_clipboard', { text })
};

// Audio file helpers
export function getAudioSrc(filePath: string): string {
	return convertFileSrc(filePath, 'asset');
}

// Format duration in mm:ss or hh:mm:ss
export function formatDuration(ms: number | null): string {
	if (ms === null || ms === undefined) return '--:--';

	const totalSeconds = Math.floor(ms / 1000);
	const hours = Math.floor(totalSeconds / 3600);
	const minutes = Math.floor((totalSeconds % 3600) / 60);
	const seconds = totalSeconds % 60;

	if (hours > 0) {
		return `${hours}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
	}
	return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

// Format file size
export function formatFileSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

// Format sample rate
export function formatSampleRate(hz: number | null): string {
	if (hz === null || hz === undefined) return '--';
	return `${(hz / 1000).toFixed(1)} kHz`;
}

// Format bit rate
export function formatBitRate(bps: number | null): string {
	if (bps === null || bps === undefined) return '--';
	return `${Math.round(bps / 1000)} kbps`;
}
