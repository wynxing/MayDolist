import { describe, expect, it } from "vitest";
import { NOTE_COLORS, noteColorId } from "./noteColor";

describe("NOTE_COLORS", () => {
  it("keeps the five stable color ids in display order", () => {
    expect(NOTE_COLORS.map((color) => color.id)).toEqual([
      "blue",
      "purple",
      "yellow",
      "green",
      "rose",
    ]);
  });
});

describe("noteColorId", () => {
  it("accepts every known color id", () => {
    for (const color of NOTE_COLORS) {
      expect(noteColorId(color.id)).toBe(color.id);
    }
  });

  it("falls back to blue for invalid, null or undefined values", () => {
    expect(noteColorId("red")).toBe("blue");
    expect(noteColorId("BLUE")).toBe("blue");
    expect(noteColorId("")).toBe("blue");
    expect(noteColorId(null)).toBe("blue");
    expect(noteColorId(undefined)).toBe("blue");
  });
});
