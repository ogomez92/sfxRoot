// Shared type definitions for Tauri IPC communication

export interface Directory {
	id: number;
	path: string;
	fileCount: number;
	lastSyncedAt: number | null;
	createdAt: number;
}

export interface IncompleteDirectory {
	id: number;
	path: string;
	indexedCount: number;
}

export interface SoundFile {
	id: number;
	directoryId: number;
	relativePath: string;
	filename: string;
	fullPath: string;
	extension: string;
	fileSize: number;
	modifiedAt: number;
	durationMs: number | null;
	sampleRate: number | null;
	channels: number | null;
	bitRate: number | null;
	codec: string | null;
	title: string | null;
	artist: string | null;
	album: string | null;
	genre: string | null;
	comment: string | null;
	indexedAt: number;
}

export interface QueryOptions {
	query?: string;
	directoryId?: number;
	minDurationMs?: number;
	maxDurationMs?: number;
	sortBy?: 'filename' | 'duration' | 'modifiedAt' | 'size';
	sortOrder?: 'asc' | 'desc';
	limit?: number;
	offset?: number;
}

export interface ResyncStats {
	unchanged: number;
	modified: number;
	added: number;
	deleted: number;
}

export interface IndexingProgress {
	phase: 'discovering' | 'scanning' | 'comparing' | 'extracting' | 'saving';
	current: number;
	total: number;
	currentFile?: string | null;
	stats?: ResyncStats | null;
}

export interface IndexingResult {
	directoryId: number;
	filesProcessed: number;
	filesAdded: number;
	filesUpdated: number;
	filesDeleted: number;
	errors: number;
	cancelled: boolean;
}

export interface FolderScanFile {
	filename: string;
	relativePath: string;
	fullPath: string;
	extension: string;
	fileSize: number;
	modifiedAt: number;
	durationMs: number | null;
	sampleRate: number | null;
	channels: number | null;
	bitRate: number | null;
	codec: string | null;
	title: string | null;
	artist: string | null;
	album: string | null;
	genre: string | null;
	comment: string | null;
}

export interface FolderScanProgress {
	scanned: number;
	currentFile: string;
}

// Supported audio extensions (Pure Rust - no FFmpeg)
export const AUDIO_EXTENSIONS = [
	'.wav',
	'.mp3',
	'.ogg',
	'.flac',
	'.aiff',
	'.aif',
	'.m4a',
	'.opus',
	'.aac'
] as const;

export type AudioExtension = (typeof AUDIO_EXTENSIONS)[number];
