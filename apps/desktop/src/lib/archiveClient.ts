import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ArchiveSession, BackendCapabilities, CreateTaskRequest, EntryPage, ExtractTaskRequest, ScanResult, TaskEvent, TaskSnapshot } from "../contracts/archive";

const isTauri = "__TAURI_INTERNALS__" in window;
async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> { return invoke<T>(name, args); }
export const archiveClient = {
  isTauri,
  capabilities: () => command<BackendCapabilities>("get_backend_capabilities"),
  pickInputPaths: (archivesOnly = false) => command<string[]>("pick_input_paths", { archivesOnly }),
  scan: (paths: string[]) => command<ScanResult>("scan_input_paths", { paths }),
  prepare: (archive: string, password?: string) => command<ArchiveSession>("prepare_archive_session", { archive, password }),
  entries: (sessionId: string, directory?: string, search?: string, offset = 0) => command<EntryPage>("list_archive_entries", { sessionId, directory, search, offset }),
  close: (sessionId: string) => command<void>("close_archive_session", { sessionId }),
  create: (request: CreateTaskRequest) => command<TaskSnapshot>("create_archive_task", { request }),
  extract: (request: ExtractTaskRequest) => command<TaskSnapshot>("extract_archive_task", { request }),
  test: (archive: string, password?: string) => command<TaskSnapshot>("test_archive_task", { archive, password }),
  update: (request: { archive: string; inputs: string[]; password?: string }) => command<TaskSnapshot>("update_archive_task", { request }),
  tasks: () => command<TaskSnapshot[]>("get_tasks"),
  cancel: (taskId: string) => command<void>("cancel_task", { taskId }),
  retry: (taskId: string, password?: string) => command<TaskSnapshot>("retry_task", { taskId, password }),
  clearCompleted: () => command<void>("clear_completed_tasks"),
  open: (path: string) => command<void>("open_path", { path }),
  reveal: (path: string) => command<void>("reveal_in_file_manager", { path }),
  onTaskEvent: async (handler: (event: TaskEvent) => void) => listen<TaskEvent>("qzip://task-event", (event) => handler(event.payload))
};
