/** Public Rust/Tauri M1 contract. Update beside archive-core models. */
export type ArchiveFormat = "sevenZip" | "zip" | "rar";
export type CompressionProfile = "fast" | "balanced" | "small";
export type ConflictPolicy = "rename" | "overwrite" | "skip" | "ask";
export type TaskStatus = "queued" | "running" | "completed" | "failed" | "cancelled";
export type ArchiveErrorCode =
  | "BACKEND_UNAVAILABLE" | "WRONG_PASSWORD" | "CORRUPT_ARCHIVE" | "DISK_FULL"
  | "FILE_IN_USE" | "ACCESS_DENIED" | "INVALID_OPTION" | "UNSUPPORTED_OPTION"
  | "UNSAFE_PATH" | "CANCELLED" | "UNKNOWN";

export interface BackendCapabilities {
  backendId: string;
  version: string;
  writableFormats: ArchiveFormat[];
  readableFormats: ArchiveFormat[];
  supportsPassword: boolean;
  supportsHeaderEncryption: boolean;
  supportsProgress: boolean;
  supportsCancellation: boolean;
}

export interface ArchiveEntry { path: string; size: number; compressedSize?: number; isDirectory: boolean; }
export interface TaskProgress { operation: "create" | "extract" | "list" | "test"; percent?: number; detail: string; }
export interface CommandError { code: ArchiveErrorCode; message: string; }
