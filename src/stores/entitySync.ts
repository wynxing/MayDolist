// Shared `entity-changed` subscription plumbing for the stores. Each store
// creates one syncer with a domain predicate and its refresh function; the
// syncer guarantees a single global listener registration and debounces
// rapid events (e.g. drag reorder) into one refresh.
import { listen } from "@tauri-apps/api/event";

const DEBOUNCE_MS = 150;

let listenerRegistered = false;
type Entry = { matches: (domain: string) => boolean; run: () => void };
const entries: Entry[] = [];

export class EntitySyncer {
  private timer: number | undefined;
  private started = false;

  constructor(
    private matches: (domain: string) => boolean,
    private refresh: () => Promise<void>
  ) {}

  /** Register the shared listener once, then run the first refresh.
   *  Repeated calls only refresh (the listener is never duplicated). */
  async init() {
    if (!this.started) {
      this.started = true;
      if (!listenerRegistered) {
        listenerRegistered = true;
        await listen<{ domain: string }>("entity-changed", (e) => {
          for (const entry of entries) {
            if (entry.matches(e.payload.domain)) entry.run();
          }
        });
      }
      entries.push({
        matches: this.matches,
        run: () => this.schedule(),
      });
    }
    await this.refresh();
  }

  private schedule() {
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => void this.refresh(), DEBOUNCE_MS);
  }
}
