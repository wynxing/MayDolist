import { invoke } from "@tauri-apps/api/core";

/** Error codes emitted by the Rust backend (see src-tauri/src/error.rs). */
export type ErrorCode =
  | "INVALID_INPUT"
  | "NOT_FOUND"
  | "CORRUPT_FILE"
  | "GITHUB_ERROR"
  | "STORAGE_ERROR"
  | "INTERNAL_ERROR"
  | "UNKNOWN";

export class ApiError extends Error {
  readonly code: ErrorCode;
  readonly recoverable: boolean;

  constructor(message: string, code: ErrorCode = "UNKNOWN", recoverable = false) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.recoverable = recoverable;
  }

  /** Surface the backend message directly when callers use String(err). */
  toString(): string {
    return this.message;
  }
}

/** Shape of the payload serialized by Rust's AppError when a command fails. */
type BackendError = { code?: unknown; message?: unknown; recoverable?: unknown };

const KNOWN_CODES: readonly string[] = [
  "INVALID_INPUT",
  "NOT_FOUND",
  "CORRUPT_FILE",
  "GITHUB_ERROR",
  "STORAGE_ERROR",
  "INTERNAL_ERROR",
];

function isBackendError(err: unknown): err is BackendError & { code: string; message: string } {
  if (typeof err !== "object" || err === null) return false;
  const candidate = err as BackendError;
  return typeof candidate.code === "string" && typeof candidate.message === "string";
}

export function normalizeError(err: unknown): ApiError {
  if (err instanceof ApiError) return err;
  if (err instanceof Error) return new ApiError(err.message);
  if (typeof err === "string") return new ApiError(err);
  if (isBackendError(err)) {
    const code = KNOWN_CODES.includes(err.code) ? (err.code as ErrorCode) : "UNKNOWN";
    return new ApiError(err.message, code, err.recoverable === true);
  }
  return new ApiError(String(err));
}

/** Single entry point for every `invoke` call (see docs/architecture.md). */
export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw normalizeError(err);
  }
}
