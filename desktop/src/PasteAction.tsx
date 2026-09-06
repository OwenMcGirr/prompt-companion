import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ProbeStatus } from "./InsertionProbe";
export type PasteRequest = {
  kind: "status" | "cancel" | "armPaste";
  text?: string;
  revision?: number;
  task_id?: string | null;
};
const nativeCall = (request: PasteRequest) =>
  invoke<ProbeStatus>("insertion_probe", { request });
export default function PasteAction({
  text,
  revision,
  taskId,
  ready,
  onCopy,
  call = nativeCall,
}: {
  text: string;
  revision: number;
  taskId: string | null;
  ready: boolean;
  onCopy: () => void;
  call?: (request: PasteRequest) => Promise<ProbeStatus>;
}) {
  const [status, setStatus] = useState<ProbeStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const flight = useRef(false);
  async function request(action: PasteRequest) {
    if (flight.current) return;
    flight.current = true;
    setBusy(true);
    try {
      setStatus(await call(action));
      setError("");
    } catch (e) {
      setError(
        `Could not confirm paste status: ${String(e)}. Check the destination before trying again.`,
      );
    } finally {
      flight.current = false;
      setBusy(false);
    }
  }
  useEffect(() => {
    void request({ kind: "status" });
  }, []);
  useEffect(() => {
    if (!status?.click_armed) return;
    const timer = setInterval(() => void request({ kind: "status" }), 250);
    return () => clearInterval(timer);
  }, [status?.click_armed]);
  if (!status?.click_available)
    return (
      <button className="primary" disabled={!text.trim()} onClick={onCopy}>
        Copy Prompt
      </button>
    );
  return (
    <div className="paste-action">
      <button
        className="primary"
        disabled={busy || !ready || (!status.click_armed && !text.trim())}
        onClick={() =>
          request(
            status.click_armed
              ? { kind: "cancel" }
              : { kind: "armPaste", text, revision, task_id: taskId },
          )
        }
      >
        {status.click_armed
          ? "Cancel waiting paste"
          : "Paste on next field click"}
      </button>
      <p>
        Click here, then click the Codex draft within 30 seconds. Uses the
        clipboard. Keeps your draft and never sends.
      </p>
      <p role="status">
        {error ||
          (status.enabled ? status.message : "Ready to paste into Codex.")}
      </p>
      <details>
        <summary>Copy instead</summary>
        <button
          disabled={busy || status.click_armed || !text.trim()}
          onClick={onCopy}
        >
          Copy Prompt
        </button>
        <p>Copying clears this draft. Undo restores it.</p>
      </details>
    </div>
  );
}
