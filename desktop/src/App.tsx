import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import type { Action, Draft, View, Settings } from "./types";
import { bridge as nativeBridge } from "./bridge";
import type { Bridge } from "./bridge";
import "./style.css";
import PasteAction from "./PasteAction";
export default function App({ bridge = nativeBridge }: { bridge?: Bridge }) {
  const [view, setView] = useState<View | null>(null),
    [draft, setDraft] = useState<Draft>({
      text: "",
      cursor: 0,
      selectionLength: 0,
    }),
    [dialog, setDialog] = useState<"tasks" | "settings" | null>(null),
    [search, setSearch] = useState(""),
    [failure, setFailure] = useState("");
  const editor = useRef<HTMLTextAreaElement>(null),
    modal = useRef<HTMLDialogElement>(null),
    sequence = useRef(0),
    pendingDraft = useRef(0),
    composing = useRef(false),
    localDraft = useRef(draft),
    queue = useRef(Promise.resolve()),
    lastFocus = useRef(-1),
    restore = useRef(false),
    focusAfter = useRef(false),
    received = useRef(0);
  const send = (action: Action) => {
    const seq = ++sequence.current;
    if (action.type === "edit") pendingDraft.current = seq;
    queue.current = queue.current
      .then(() => bridge.send(action, seq))
      .catch((e) => {
        setFailure(String(e));
      });
  };
  useEffect(() => {
    let disposed = false,
      unlisten: undefined | (() => void);
    const apply = (next: View) => {
      if (disposed) return;
      sequence.current = Math.max(sequence.current, next.acknowledged);
      setView(next);
      if (!composing.current && next.acknowledged >= pendingDraft.current) {
        setDraft(next.draft);
        localDraft.current = next.draft;
        if (lastFocus.current !== next.focus) {
          lastFocus.current = next.focus;
          focusAfter.current = true;
        }
      }
    };
    bridge
      .subscribe((next) => {
        received.current++;
        apply(next);
      })
      .then((stop) => {
        if (disposed) {
          stop();
          return;
        }
        unlisten = stop;
        const before = received.current;
        bridge
          .snapshot()
          .then((next) => {
            if (received.current === before) apply(next);
          })
          .catch((e) => setFailure(String(e)));
      })
      .catch((e) => setFailure(String(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [bridge]);
  useLayoutEffect(() => {
    if (
      !editor.current ||
      composing.current ||
      dialog ||
      !view ||
      view.acknowledged < pendingDraft.current
    )
      return;
    restore.current = true;
    editor.current.setSelectionRange(
      draft.cursor,
      draft.cursor + draft.selectionLength,
    );
    if (focusAfter.current) {
      editor.current.focus();
      focusAfter.current = false;
    }
    queueMicrotask(() => {
      restore.current = false;
    });
  }, [draft, view?.focus, dialog]);
  useEffect(() => {
    if (dialog) modal.current?.showModal();
    else modal.current?.close();
  }, [dialog]);
  useEffect(() => {
    if (dialog !== "tasks") return;
    const timer = setTimeout(
      () => send({ type: "tasks", search, more: false }),
      400,
    );
    return () => clearTimeout(timer);
  }, [search, dialog]);
  function edit(element: HTMLTextAreaElement) {
    const d = {
      text: element.value,
      cursor: element.selectionStart,
      selectionLength: element.selectionEnd - element.selectionStart,
    };
    localDraft.current = d;
    setDraft(d);
    if (!composing.current) send({ type: "edit", draft: d });
  }
  function selection(element: HTMLTextAreaElement) {
    if (
      restore.current ||
      composing.current ||
      document.activeElement !== element
    )
      return;
    const d = {
      text: element.value,
      cursor: element.selectionStart,
      selectionLength: element.selectionEnd - element.selectionStart,
    };
    const old = localDraft.current;
    if (
      d.cursor !== old.cursor ||
      d.selectionLength !== old.selectionLength ||
      d.text !== old.text
    ) {
      localDraft.current = d;
      setDraft(d);
      send({ type: "edit", draft: d });
    }
  }
  function settings(change: Partial<Settings>) {
    if (view)
      send({ type: "settings", settings: { ...view.settings, ...change } });
  }
  if (!view)
    return (
      <main>
        <h1>Prompt Companion Preview</h1>
        <p role="status">{failure || "Opening your composer…"}</p>
      </main>
    );
  const expansion =
    view.phase === "expanding" || view.phase === "clarification";
  const rowLabels =
    view.clarification?.choices ??
    (view.active
      ? [
          "Suggestions paused while this task is active",
          "You can still edit or copy your draft",
          "Suggestions resume when the task finishes",
        ]
      : expansion
        ? [
            "Preparing your expansion…",
            "Your original draft is safe",
            "Use Keep original to cancel",
          ]
        : view.phrases.length
          ? view.phrases
          : [
              "Choose a task to begin",
              "Suggestions use its conversation",
              "Click a phrase to add it",
            ]);
  const style = {
    "--draft-size": `${view.settings.fontSize}px`,
    "--button-height": `${Math.max(view.settings.buttonHeight, Math.ceil(view.settings.fontSize * 3.6))}px`,
  } as CSSProperties;
  return (
    <main style={style}>
      <header>
        <div>
          <h1>
            Prompt Companion <span className="badge">Preview</span>
          </h1>
          <p>Your words, a little sooner.</p>
        </div>
        <button aria-label="Settings" onClick={() => setDialog("settings")}>
          ⚙
        </button>
      </header>
      <button
        className="context"
        onClick={() => {
          setDialog("tasks");
          send({ type: "tasks", search, more: false });
        }}
      >
        <small>CONTEXT FROM</small>
        <strong>{view.selected?.title || "Choose a Codex task"}</strong>
        <span aria-hidden>⌄</span>
      </button>
      <div className="draft-heading">
        <label htmlFor="draft">YOUR PROMPT</label>
        <button
          disabled={!view.canExpand}
          onClick={() => send({ type: "expand", revision: view.revision })}
        >
          Expand
        </button>
      </div>
      <textarea
        id="draft"
        ref={editor}
        aria-label="Prompt draft"
        placeholder="Start typing, or choose a phrase below…"
        spellCheck={false}
        value={draft.text}
        onChange={(e) => edit(e.currentTarget)}
        onSelect={(e) => selection(e.currentTarget)}
        onCompositionStart={() => {
          composing.current = true;
        }}
        onCompositionEnd={(e) => {
          composing.current = false;
          edit(e.currentTarget);
        }}
        onKeyDown={(e) => {
          if (
            (e.metaKey || e.ctrlKey) &&
            e.key.toLowerCase() === "z" &&
            !e.shiftKey
          ) {
            e.preventDefault();
            send({ type: "undo" });
          }
        }}
      />
      <div className="suggestion-heading">
        <strong>
          {view.clarification?.question ||
            (expansion ? "EXPANDING YOUR WORDS" : "CONTINUE WITH")}
        </strong>
        <button
          disabled={!view.selected || view.connecting || view.active}
          onClick={() => send({ type: expansion ? "keepOriginal" : "refresh" })}
        >
          {expansion ? "Keep original" : "Refresh phrases"}
        </button>
      </div>
      <div
        className="phrases"
        onPointerEnter={() => send({ type: "hover", value: true })}
        onPointerLeave={() => send({ type: "hover", value: false })}
      >
        {[0, 1, 2].map((i) => {
          const label = rowLabels[i] ?? "Or keep your original words";
          const enabled =
            !!view.clarification?.choices[i] ||
            (view.canInsert && !!view.phrases[i]);
          return (
            <button
              className="phrase"
              key={i}
              disabled={!enabled}
              aria-label={
                enabled
                  ? `${view.clarification ? "Choose meaning" : "Insert"}: ${label}`
                  : label
              }
              onClick={() =>
                send({
                  type: view.clarification ? "choose" : "insert",
                  index: i,
                  revision: view.revision,
                })
              }
            >
              <span className="number" aria-hidden>
                {i + 1}
              </span>
              <span>{label}</span>
              <span aria-hidden>+</span>
            </button>
          );
        })}
      </div>
      <div className="status">
        <p role="status">
          {failure || view.storageProblem || view.problem || view.status}
        </p>
        {(!view.connected || view.problem) && (
          <button
            disabled={view.connecting}
            onClick={() => send({ type: "reconnect" })}
          >
            Reconnect
          </button>
        )}
      </div>
      <div className="actions">
        <button
          disabled={!view.undoAvailable}
          onClick={() => send({ type: "undo" })}
        >
          ↶ Undo
        </button>
        <button disabled={!draft.text} onClick={() => send({ type: "clear" })}>
          Clear
        </button>
        {bridge === nativeBridge ? (
          <PasteAction
            text={draft.text}
            revision={view.revision}
            taskId={view.selected?.id ?? null}
            ready={view.acknowledged >= sequence.current}
            onCopy={() => send({ type: "copy" })}
          />
        ) : (
          <button
            className="primary"
            disabled={!draft.text.trim()}
            onClick={() => send({ type: "copy" })}
          >
            {view.copied ? "✓ Copied" : "Copy Prompt"}
          </button>
        )}
      </div>
      <footer>
        <p>{view.contextStatus}</p>
        <p>You choose where to paste. Nothing sends automatically.</p>
      </footer>
      <dialog
        ref={modal}
        aria-label={dialog === "tasks" ? "Choose your context" : "Settings"}
        onCancel={() => setDialog(null)}
      >
        {dialog === "tasks" ? (
          <>
            <h2>Choose your context</h2>
            <p>Pick the task you’re writing a prompt for.</p>
            <label>
              Search tasks
              <input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </label>
            <div className="task-list">
              {view.loadingTasks && <p role="status">Loading tasks…</p>}
              {view.tasks.map((t) => (
                <button
                  key={t.id}
                  onClick={() => {
                    send({ type: "select", id: t.id });
                    setDialog(null);
                  }}
                >
                  {t.title}
                </button>
              ))}
              {!view.tasks.length && !view.loadingTasks && (
                <p>No matching local tasks.</p>
              )}
              {view.more && (
                <button
                  onClick={() => send({ type: "tasks", search, more: true })}
                >
                  Load more
                </button>
              )}
            </div>
          </>
        ) : (
          <>
            <h2>Make it comfortable</h2>
            <label>
              Text size · {view.settings.fontSize} pt
              <input
                aria-label="Text size"
                type="range"
                min="18"
                max="32"
                step="1"
                value={view.settings.fontSize}
                onChange={(e) => settings({ fontSize: +e.target.value })}
              />
            </label>
            <label>
              Phrase button height · {view.settings.buttonHeight} pt
              <input
                aria-label="Phrase button height"
                type="range"
                min="72"
                max="120"
                step="2"
                value={view.settings.buttonHeight}
                onChange={(e) => settings({ buttonHeight: +e.target.value })}
              />
            </label>
            <label className="toggle">
              <input
                type="checkbox"
                checked={view.settings.automatic}
                onChange={(e) => settings({ automatic: e.target.checked })}
              />
              Suggest automatically as I type
            </label>
            <label className="toggle">
              <input
                type="checkbox"
                checked={view.settings.floating}
                onChange={(e) => settings({ floating: e.target.checked })}
              />
              Keep this window above other apps
            </label>
            <hr />
            <h3>This session</h3>
            <p>
              {view.typed} characters typed · {view.inserted} inserted
              <br />
              {view.accepted} phrases selected
            </p>
            {view.latency !== null && (
              <p>Last generation: {view.latency.toFixed(1)} seconds</p>
            )}
            <hr />
            <p className="secondary">
              Uses your Codex ChatGPT sign-in and allowance. The selected
              conversation and draft go to OpenAI for generation. Drafts stay
              saved on this computer.
            </p>
            <p className="secondary">
              Phrases: {view.model}
              <br />
              Expansion: {view.expansionModel}
            </p>
            <p className="secondary">
              Direct insertion is not enabled in this preview.
            </p>
          </>
        )}
        <button className="done" onClick={() => setDialog(null)}>
          Done
        </button>
      </dialog>
    </main>
  );
}
