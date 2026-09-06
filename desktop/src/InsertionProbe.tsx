import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
export type ProbeStatus = {
  available: boolean;
  enabled: boolean;
  armed: boolean;
  manual_codex: boolean;
  manual_codex_available: boolean;
  token: number;
  destination: string | null;
  message: string;
};
type Request = { kind: string; value?: boolean; text?: string; token?: number };
const nativeCall = (request: Request) =>
  invoke<ProbeStatus>("insertion_probe", { request });
export default function InsertionProbe({
  text,
  call = nativeCall,
  onUseTestText,
}: {
  text: string;
  call?: (request: Request) => Promise<ProbeStatus>;
  onUseTestText?: () => void;
}) {
  const [status, setStatus] = useState<ProbeStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const inFlight = useRef(false);
  async function request(action: Request) {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    try {
      setStatus(await call(action));
    } catch (e) {
      setStatus((s) =>
        s
          ? {
              ...s,
              armed: false,
              destination: null,
              message: `Outcome unavailable: ${String(e)}. Inspect the destination before capturing again. No retry was made.`,
            }
          : null,
      );
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  useEffect(() => {
    void request({ kind: "status" });
  }, []);
  useEffect(() => {
    if (!status?.enabled || !status.armed) return;
    const timer = setInterval(() => {
      // Native capture ignores our own process. Web-view focus can remain true
      // after the OS activates another app, so it must not gate native capture.
      void request({ kind: "capture" });
    }, 200);
    return () => clearInterval(timer);
  }, [status?.enabled, status?.armed]);
  if (!status?.available) return null;
  return (
    <section className="prototype">
      <h3>Manual insertion test</h3>
      <p>
        Development build only. These methods are not approved for everyday use.
        Nothing presses Enter or sends a prompt.
      </p>
      <label className="toggle">
        <input
          type="checkbox"
          checked={status.enabled}
          disabled={busy}
          onChange={(e) => request({ kind: "enable", value: e.target.checked })}
        />
        Enable insertion testing
      </label>
      {status.manual_codex_available && (
        <label className="toggle">
          <input
            type="checkbox"
            checked={status.manual_codex}
            disabled={busy}
            onChange={(e) =>
              request({ kind: "manualCodex", value: e.target.checked })
            }
          />
          Include Codex — I will operate the test myself
        </label>
      )}
      <p>
        {status.manual_codex
          ? "Codex is allowed for this session’s manual test. You must click its draft field and inspect the result yourself."
          : "Only TextEdit/Notepad and Chrome are eligible. Codex is excluded until manual testing is selected on macOS."}
      </p>
      {onUseTestText && (
        <button disabled={busy} onClick={onUseTestText}>
          Use TEST in my draft (Undo available)
        </button>
      )}
      <ol>
        <li>
          Put disposable text in the destination. For a first test, use “LEFT
          RIGHT” and place the cursor just before RIGHT.
        </li>
        <li>
          Write “TEST ” in this draft. Click Capture next field, then click the
          destination’s cursor position yourself.
        </li>
        <li>
          Return here after capture. Confirm the application below, then click
          Insert natively once.
        </li>
        <li>
          Inspect the destination without sending. Expect “LEFT TEST RIGHT”.
          Check its Undo using the destination’s Edit menu.
        </li>
      </ol>
      <button
        disabled={busy || !status.enabled || status.armed}
        onClick={() => request({ kind: "arm" })}
      >
        {status.armed ? "Waiting for your field…" : "Capture next field"}
      </button>
      <p>
        Destination: <strong>{status.destination || "None captured"}</strong>
      </p>
      <p role="status">{status.message}</p>
      <p>
        Paste with clipboard replaces plain-text clipboard contents. Unsupported
        formats are left untouched. No timed restoration is used. Your draft
        stays here.
      </p>
      <div className="actions">
        {["native", "paste"].map((kind) => (
          <button
            key={kind}
            disabled={
              busy || !text.trim() || !status.destination || !status.enabled
            }
            onClick={() => request({ kind, text, token: status.token })}
          >
            {kind === "native" ? "Insert natively" : "Paste with clipboard"}
          </button>
        ))}
      </div>
      <p>
        Every attempt consumes its target. If the result is uncertain, inspect
        the destination before doing anything else. A later clipboard trial
        requires a fresh capture and your explicit click.
      </p>
    </section>
  );
}
