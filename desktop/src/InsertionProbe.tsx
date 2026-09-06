import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
type Status = {
  available: boolean;
  enabled: boolean;
  destination: string | null;
  message: string;
};
export default function InsertionProbe({ text }: { text: string }) {
  const [status, setStatus] = useState<Status | null>(null),
    [busy, setBusy] = useState(false);
  async function request(request: object) {
    try {
      setStatus(await invoke<Status>("insertion_probe", { request }));
    } catch (e) {
      setStatus((s) => (s ? { ...s, message: String(e) } : null));
    }
  }
  useEffect(() => {
    void request({ kind: "status" });
  }, []);
  useEffect(() => {
    if (!status?.enabled) return;
    let pending = false;
    const timer = setInterval(() => {
      if (document.hasFocus() || pending) return;
      pending = true;
      request({ kind: "capture" }).finally(() => {
        pending = false;
      });
    }, 200);
    return () => clearInterval(timer);
  }, [status?.enabled]);
  if (!status?.available) return null;
  return (
    <section className="prototype">
      <h3>Development insertion experiment</h3>
      <label className="toggle">
        <input
          type="checkbox"
          checked={status.enabled}
          onChange={(e) => request({ kind: "enable", value: e.target.checked })}
        />
        Enable external-field detection
      </label>
      <p>
        Only disposable TextEdit/Notepad and Chrome fields are allowed. Codex is
        not validated. No automatic submission.
      </p>
      <p>
        Destination: <strong>{status.destination || "None selected"}</strong>
      </p>
      <p role="status">{status.message}</p>
      <p>
        Paste uses the clipboard and replaces its plain text. Unsupported
        clipboard formats are left untouched. Your draft stays here.
      </p>
      <div className="actions">
        {["native", "paste"].map((kind) => (
          <button
            key={kind}
            disabled={
              busy || !text.trim() || !status.destination || !status.enabled
            }
            onClick={async () => {
              setBusy(true);
              await request({ kind, text });
              setBusy(false);
            }}
          >
            {kind === "native" ? "Insert natively" : "Paste with clipboard"}
          </button>
        ))}
      </div>
    </section>
  );
}
