export const NOTE_COLORS = [
  { id: "blue", label: "蓝" },
  { id: "purple", label: "紫" },
  { id: "yellow", label: "黄" },
  { id: "green", label: "绿" },
  { id: "rose", label: "玫" },
] as const;

export type NoteColorId = (typeof NOTE_COLORS)[number]["id"];

export function noteColorId(value: string | null | undefined): NoteColorId {
  return NOTE_COLORS.some((color) => color.id === value)
    ? (value as NoteColorId)
    : "blue";
}
