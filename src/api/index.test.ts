import { describe, expect, it, vi } from "vitest";
import { ApiError, call, normalizeError } from "./index";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = vi.mocked(invoke);

describe("normalizeError", () => {
  it("passes ApiError through unchanged", () => {
    const original = new ApiError("boom", "STORAGE_ERROR", true);
    expect(normalizeError(original)).toBe(original);
  });

  it("wraps plain Error with its message", () => {
    const err = normalizeError(new Error("plain failure"));
    expect(err).toBeInstanceOf(ApiError);
    expect(err.message).toBe("plain failure");
    expect(err.code).toBe("UNKNOWN");
    expect(err.recoverable).toBe(false);
  });

  it("wraps string errors", () => {
    const err = normalizeError("string failure");
    expect(err.message).toBe("string failure");
    expect(err.code).toBe("UNKNOWN");
  });

  // The Rust backend serializes AppError as { code, message, recoverable }
  // (src-tauri/src/error.rs). Every variant must surface its message and
  // keep its code instead of degrading to "[object Object]".
  const backendCases: Array<{
    code: string;
    message: string;
    recoverable: boolean;
  }> = [
    { code: "INVALID_INPUT", message: "invalid input: title empty", recoverable: true },
    { code: "NOT_FOUND", message: "not found: todo list", recoverable: true },
    { code: "CORRUPT_FILE", message: "corrupt JSON at todos.json", recoverable: true },
    { code: "GITHUB_ERROR", message: "github error: gh exited 1", recoverable: true },
    { code: "STORAGE_ERROR", message: "storage error: disk full", recoverable: true },
    { code: "INTERNAL_ERROR", message: "internal error: lock poisoned", recoverable: false },
  ];

  it.each(backendCases)(
    "preserves structured backend error $code",
    ({ code, message, recoverable }) => {
      const err = normalizeError({ code, message, recoverable });
      expect(err.message).toBe(message);
      expect(err.code).toBe(code);
      expect(err.recoverable).toBe(recoverable);
      // String(err) must surface the message directly for UI display.
      expect(String(err)).toBe(message);
    }
  );

  it("falls back to UNKNOWN for unrecognized backend codes", () => {
    const err = normalizeError({ code: "SOMETHING_NEW", message: "mystery", recoverable: true });
    expect(err.code).toBe("UNKNOWN");
    expect(err.message).toBe("mystery");
  });

  it("degrades unknown objects via String()", () => {
    const err = normalizeError({ unexpected: true });
    expect(err.message).toBe("[object Object]");
    expect(err.code).toBe("UNKNOWN");
  });

  it("does not treat Error instances as backend payloads", () => {
    const err = normalizeError(new Error("not a payload"));
    expect(err.code).toBe("UNKNOWN");
  });
});

describe("call", () => {
  it("wraps backend rejection into ApiError", async () => {
    mockedInvoke.mockRejectedValueOnce({
      code: "NOT_FOUND",
      message: "not found: note",
      recoverable: true,
    });
    await expect(call("note_get", { id: "x" })).rejects.toMatchObject({
      name: "ApiError",
      message: "not found: note",
      code: "NOT_FOUND",
    });
  });

  it("returns invoke result untouched on success", async () => {
    mockedInvoke.mockResolvedValueOnce({ ok: true });
    await expect(call("some_command")).resolves.toEqual({ ok: true });
  });
});
