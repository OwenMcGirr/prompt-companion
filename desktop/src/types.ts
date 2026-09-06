export type Draft = { text: string; cursor: number; selectionLength: number };
export type Settings = {
  fontSize: number;
  buttonHeight: number;
  automatic: boolean;
  floating: boolean;
};
export type TaskInfo = { id: string; title: string };
export type View = {
  draft: Draft;
  selected: TaskInfo | null;
  tasks: TaskInfo[];
  more: boolean;
  loadingTasks: boolean;
  settings: Settings;
  revision: number;
  acknowledged: number;
  focus: number;
  connected: boolean;
  connecting: boolean;
  status: string;
  problem: string | null;
  storageProblem: string | null;
  contextStatus: string;
  active: boolean;
  phrases: string[];
  canInsert: boolean;
  canExpand: boolean;
  phase: "idle" | "predicting" | "expanding" | "clarification";
  clarification: { question: string; choices: string[] } | null;
  copied: boolean;
  undoAvailable: boolean;
  typed: number;
  inserted: number;
  accepted: number;
  latency: number | null;
  model: string;
  expansionModel: string;
};
export type Action =
  | { type: "edit"; draft: Draft }
  | { type: "select"; id: string }
  | { type: "insert" | "choose"; index: number; revision: number }
  | { type: "expand"; revision: number }
  | { type: "hover"; value: boolean }
  | { type: "tasks"; search: string; more: boolean }
  | { type: "settings"; settings: Settings }
  | {
      type:
        "undo" | "clear" | "copy" | "keepOriginal" | "refresh" | "reconnect";
    };
