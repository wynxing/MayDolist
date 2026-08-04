import { invoke } from "@tauri-apps/api/core";

export class ApiError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ApiError";
  }
}

export function normalizeError(err: unknown): ApiError {
  if (err instanceof ApiError) return err;
  if (err instanceof Error) return new ApiError(err.message);
  if (typeof err === "string") return new ApiError(err);
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
