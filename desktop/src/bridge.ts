import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Action, View } from "./types";
export interface Bridge {
  snapshot: () => Promise<View>;
  subscribe: (fn: (view: View) => void) => Promise<() => void>;
  send: (action: Action, sequence: number) => Promise<void>;
}
export const bridge: Bridge = {
  snapshot: () => invoke("snapshot"),
  subscribe: (fn) => listen<View>("view", (event) => fn(event.payload)),
  send: (action, sequence) =>
    invoke("action", { request: { sequence, action } }),
};
