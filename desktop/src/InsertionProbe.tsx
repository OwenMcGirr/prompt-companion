import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
export type ProbeStatus = {
  available: boolean;
  enabled: boolean;
  armed: boolean;
  click_armed: boolean;
  click_available: boolean;
  manual_codex: boolean;
  manual_codex_available: boolean;
  token: number;
  destination: string | null;
  message: string;
};
type Request = {
  kind: string;
  value?: boolean;
  text?: string;
  token?: number;
  clipboard?: boolean;
};
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
              click_armed: false,
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
    if (!status?.enabled || (!status.armed && !status.click_armed)) return;
    const timer = setInterval(() => {
      // Native capture ignores our own process. Web-view focus can remain true
      // after the OS activates another app, so it must not gate native capture.
      void request({ kind: status.click_armed ? "status" : "capture" });
    }, 200);
    return () => clearInterval(timer);
  }, [status?.enabled, status?.armed, status?.click_armed]);
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
          Use “TEST ” in this draft. Make the destination field visible first,
          then click Insert on next field click.
        </li>
        <li>
          Click directly before RIGHT in the destination. It will capture and
          insert once after that click, without returning here. Other clicks
          cancel the attempt. The request expires after 30 seconds.
        </li>
        <li>
          Inspect the destination without sending. Expect “LEFT TEST RIGHT”.
          Check its Undo using the destination’s Edit menu.
        </li>
      </ol>
      {status.click_available && (
        <>
          <div className="actions">
            <button
              disabled={
                busy || !status.enabled || !text.trim() || status.click_armed
              }
              onClick={() =>
                request({ kind: "armClick", text, clipboard: false })
              }
            >
              Insert on next field click
            </button>
            <button
              disabled={
                busy || !status.enabled || !text.trim() || status.click_armed
              }
              onClick={() =>
                request({ kind: "armClick", text, clipboard: true })
              }
            >
              Paste on next field click
            </button>
          </div>
          <p>
            {status.manual_codex
              ? "Next-click insertion is restricted to Codex for this trial."
              : "Next-click insertion accepts disposable TextEdit or Chrome fields."}{" "}
            The draft stays here. No Enter or automatic retry.
          </p>
        </>
      )}
      {(status.click_armed || status.armed) && (
        <button disabled={busy} onClick={() => request({ kind: "cancel" })}>
          Cancel waiting insertion
        </button>
      )}
      <p>Optional: capture first and review before inserting</p>
      <button
        disabled={busy || !status.enabled || status.armed || status.click_armed}
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
              busy ||
              status.click_armed ||
              !text.trim() ||
              !status.destination ||
              !status.enabled
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
