export type ArchiveFormat = "sevenZip" | "zip" | "tar" | "tarGz" | "tarXz" | "rar" | "gz" | "xz" | "bz2" | "iso" | "cab" | "wim" | "unknown";
export type CompressionProfile = "store" | "fast" | "balanced" | "small" | "maximum";
export type ConflictPolicy = "rename" | "overwrite" | "skip" | "ask";
export type TaskStatus = "queued" | "scanning" | "running" | "cancelling" | "completed" | "failed" | "cancelled";
export type ArchiveOperation = "create" | "extract" | "list" | "test" | "update";
export type ArchiveErrorCode = "BACKEND_UNAVAILABLE" | "FILE_NOT_FOUND" | "PERMISSION_DENIED" | "WRONG_PASSWORD" | "CORRUPT_ARCHIVE" | "DISK_FULL" | "FILE_IN_USE" | "ACCESS_DENIED" | "INVALID_OPTION" | "INVALID_REQUEST" | "UNSUPPORTED_OPTION" | "UNSUPPORTED_FORMAT" | "UNSAFE_PATH" | "ARCHIVE_BOMB_RISK" | "CONFLICT_REQUIRES_DECISION" | "CLEANUP_FAILED" | "CANCELLED" | "UNKNOWN";
export interface BackendCapabilities { backendId: string; version: string; writableFormats: ArchiveFormat[]; readableFormats: ArchiveFormat[]; supportsPassword: boolean; supportsHeaderEncryption: boolean; supportsPartialExtract: boolean; supportsUpdate: boolean; supportsProgress: boolean; supportsCancellation: boolean; }
export interface ArchiveEntry { path: string; displayName: string; size: number; compressedSize?: number; isDirectory: boolean; modifiedAt?: string; crc?: string; attributes?: string; encrypted: boolean; isSymlink: boolean; isHardlink: boolean; }
export interface ArchiveRisk { code: string; message: string; overridable: boolean; }
export interface ArchiveSession { sessionId: string; format: ArchiveFormat; compressedSize: number; estimatedUncompressedSize: number; entryCount: number; encrypted: boolean; risks: ArchiveRisk[]; }
export interface EntryPage { entries: ArchiveEntry[]; total: number; nextOffset?: number | null; }
export interface TaskProgress { phase: string; percent?: number; currentEntry?: string; elapsedSeconds: number; }
export interface ArchiveError { code: ArchiveErrorCode; message: string; recoverable: boolean; }
export interface TaskSnapshot { taskId: string; operation: ArchiveOperation; status: TaskStatus; displayName: string; output?: string; createdAt: number; updatedAt: number; progress?: TaskProgress; error?: ArchiveError; warnings: string[]; retryable: boolean; }
export interface TaskEvent { eventType: string; task: TaskSnapshot; }
export interface ScanResult { paths: string[]; archivePaths: string[]; normalPaths: string[]; totalBytes: number; }
export interface CreateTaskRequest { inputs: string[]; output: string; format: ArchiveFormat; profile: CompressionProfile; password?: string; encryptHeaders: boolean; testAfterCreate: boolean; deleteSourcesAfterSuccess: boolean; }
export interface ExtractTaskRequest { archive: string; output: string; selectedEntries?: string[]; conflictPolicy: ConflictPolicy; password?: string; acceptRisk: boolean; }
