import { call } from "./index";

export interface QuickCaptureResult {
  kind: "todo" | "note";
  id: string;
  title: string;
  targetListId: string | null;
}

export const submit = (text: string) =>
  call<QuickCaptureResult>("quick_capture_submit", { text });

export const hide = () => call<void>("quick_capture_hide");
