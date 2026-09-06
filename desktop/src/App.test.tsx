// @vitest-environment jsdom
import { afterEach, beforeAll, describe, it, expect, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
  act,
} from "@testing-library/react";
import App from "./App";
import userEvent from "@testing-library/user-event";
import type { View, Action } from "./types";
import type { Bridge } from "./bridge";
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute("open");
  };
});
afterEach(cleanup);
function fixture() {
  let view: View = {
    draft: { text: "", cursor: 0, selectionLength: 0 },
    selected: { id: "A", title: "Test task" },
    tasks: [],
    more: false,
    loadingTasks: false,
    settings: {
      fontSize: 22,
      buttonHeight: 86,
      automatic: true,
      floating: true,
    },
    revision: 2,
    acknowledged: 0,
    focus: 0,
    connected: true,
    connecting: false,
    status: "Ready",
    problem: null,
    storageProblem: null,
    contextStatus: "Conversation connected",
    active: false,
    phrases: ["Fix login", "Add tests", "Explain error"],
    canInsert: true,
    canExpand: false,
    phase: "idle",
    clarification: null,
    copied: false,
    undoAvailable: false,
    typed: 0,
    inserted: 0,
    accepted: 0,
    latency: null,
    model: "test",
    expansionModel: "test",
  };
  let callback: (v: View) => void = () => {};
  const calls: { action: Action; sequence: number }[] = [];
  const bridge: Bridge = {
    snapshot: async () => view,
    subscribe: async (fn) => {
      callback = fn;
      return () => {};
    },
    send: async (action, sequence) => {
      calls.push({ action, sequence });
    },
  };
  return {
    bridge,
    calls,
    update: (next: Partial<View>) => {
      view = { ...view, ...next };
      act(() => callback(view));
    },
  };
}
describe("composer interactions", () => {
  it("dispatches an explicit phrase selection and focuses the applied draft", async () => {
    const f = fixture();
    render(<App bridge={f.bridge} />);
    const button = await screen.findByRole("button", {
      name: "Insert: Fix login",
    });
    fireEvent.click(button);
    await waitFor(() =>
      expect(f.calls.some((c) => c.action.type === "insert")).toBe(true),
    );
    f.update({
      draft: { text: "Fix login ", cursor: 10, selectionLength: 0 },
      focus: 1,
      acknowledged: 1,
      revision: 3,
      canInsert: false,
    });
    const field = screen.getByRole("textbox", {
      name: "Prompt draft",
    }) as HTMLTextAreaElement;
    await waitFor(() => expect(field.value).toBe("Fix login "));
    expect(document.activeElement).toBe(field);
    expect(field.selectionStart).toBe(10);
  });
  it("keeps a newer local edit when an older backend snapshot arrives", async () => {
    const f = fixture();
    render(<App bridge={f.bridge} />);
    const field = (await screen.findByRole("textbox", {
      name: "Prompt draft",
    })) as HTMLTextAreaElement;
    fireEvent.change(field, {
      target: { value: "new text", selectionStart: 8, selectionEnd: 8 },
    });
    await waitFor(() => expect(f.calls.length).toBeGreaterThan(0));
    f.update({
      draft: { text: "old", cursor: 3, selectionLength: 0 },
      acknowledged: 0,
    });
    expect(field.value).toBe("new text");
  });
  it("does not send unfinished IME composition", async () => {
    const f = fixture();
    render(<App bridge={f.bridge} />);
    const field = await screen.findByRole("textbox", { name: "Prompt draft" });
    fireEvent.compositionStart(field);
    fireEvent.change(field, {
      target: { value: "漢", selectionStart: 1, selectionEnd: 1 },
    });
    await act(async () => {});
    expect(f.calls).toHaveLength(0);
    fireEvent.compositionEnd(field);
    await waitFor(() => expect(f.calls[0]?.action.type).toBe("edit"));
  });
  it("waits for copy success before displaying an empty draft", async () => {
    const f = fixture();
    render(<App bridge={f.bridge} />);
    await screen.findByRole("textbox");
    f.update({
      draft: { text: "Keep me", cursor: 7, selectionLength: 0 },
      canExpand: true,
    });
    fireEvent.click(screen.getByRole("button", { name: "Copy Prompt" }));
    await waitFor(() =>
      expect(f.calls.some((c) => c.action.type === "copy")).toBe(true),
    );
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(
      "Keep me",
    );
    f.update({ problem: "Copy failed" });
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(
      "Keep me",
    );
    f.update({
      draft: { text: "", cursor: 0, selectionLength: 0 },
      copied: true,
      focus: 1,
      undoAvailable: true,
    });
    await waitFor(() =>
      expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(
        "",
      ),
    );
    expect(document.activeElement).toBe(screen.getByRole("textbox"));
  });
  it("offers clarification and Keep original without replacing the draft", async () => {
    const f = fixture();
    render(<App bridge={f.bridge} />);
    await screen.findByRole("textbox");
    f.update({
      phase: "clarification",
      clarification: {
        question: "Which issue?",
        choices: ["Slow suggestions", "Small buttons"],
      },
      canInsert: false,
      draft: { text: "fix it", cursor: 6, selectionLength: 0 },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Choose meaning: Small buttons" }),
    );
    await waitFor(() =>
      expect(
        f.calls.some((c) => c.action.type === "choose" && c.action.index === 1),
      ).toBe(true),
    );
    fireEvent.click(screen.getByRole("button", { name: "Keep original" }));
    await waitFor(() =>
      expect(f.calls.some((c) => c.action.type === "keepOriginal")).toBe(true),
    );
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(
      "fix it",
    );
  });
  it("pauses controls for an active task without disabling the editor", async () => {
    const f = fixture();
    render(<App bridge={f.bridge} />);
    await screen.findByRole("textbox");
    f.update({ active: true, canInsert: false, canExpand: false });
    expect(
      (
        screen.getByRole("button", {
          name: "Refresh phrases",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (screen.getByRole("button", { name: "Expand" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).disabled).toBe(
      false,
    );
  });
  it("keeps full long phrases available without truncating the insertion label", async () => {
    const f = fixture();
    render(<App bridge={f.bridge} />);
    await screen.findByRole("textbox");
    const label =
      "Please keep the existing arrangement while making each of the phrase buttons easier to select with an ordinary left click.";
    f.update({
      phrases: [label],
      settings: {
        fontSize: 32,
        buttonHeight: 120,
        automatic: true,
        floating: true,
      },
    });
    expect(
      screen.getByRole("button", { name: `Insert: ${label}` }),
    ).toBeTruthy();
  });
  it("opens settings with large controls and closes with Done", async () => {
    const f = fixture();
    render(<App bridge={f.bridge} />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
    fireEvent.change(screen.getByRole("slider", { name: "Text size" }), {
      target: { value: "30" },
    });
    await waitFor(() =>
      expect(
        f.calls.some(
          (c) =>
            c.action.type === "settings" && c.action.settings.fontSize === 30,
        ),
      ).toBe(true),
    );
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

it("navigates suggestions with arrows, wraps, accepts with Enter, and returns draft focus", async () => {
  const f = fixture();
  const user = userEvent.setup();
  render(<App bridge={f.bridge} />);
  const draft = await screen.findByRole("textbox");
  draft.focus();
  await user.keyboard("{ArrowDown}");
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Insert: Fix login" }),
  );
  await user.keyboard("{ArrowLeft}");
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Insert: Explain error" }),
  );
  await user.keyboard("{ArrowRight}{ArrowDown}{Enter}");
  await waitFor(() =>
    expect(
      f.calls.filter((c) => c.action.type === "insert").map((c) => c.action),
    ).toEqual([{ type: "insert", index: 1, revision: 2 }]),
  );
  expect(document.activeElement).toBe(draft);
  expect(f.calls.some((c) => c.action.type === "hover" && c.action.value)).toBe(
    true,
  );
});
it("Escape exits navigation and paused suggestions do not capture arrows", async () => {
  const f = fixture();
  const user = userEvent.setup();
  render(<App bridge={f.bridge} />);
  const draft = await screen.findByRole("textbox");
  draft.focus();
  await user.keyboard("{ArrowDown}{Escape}");
  expect(document.activeElement).toBe(draft);
  f.update({ active: true, canInsert: false });
  await user.keyboard("{ArrowDown}{Enter}");
  expect(document.activeElement).toBe(draft);
  expect(f.calls.some((c) => c.action.type === "insert")).toBe(false);
});
it("preserves multiline caret keys, modified arrows and IME composition", async () => {
  const f = fixture();
  render(<App bridge={f.bridge} />);
  const draft = (await screen.findByRole("textbox")) as HTMLTextAreaElement;
  f.update({ draft: { text: "one\ntwo", cursor: 2, selectionLength: 0 } });
  draft.focus();
  draft.setSelectionRange(2, 2);
  expect(fireEvent.keyDown(draft, { key: "ArrowDown" })).toBe(true);
  draft.setSelectionRange(7, 7);
  expect(fireEvent.keyDown(draft, { key: "ArrowDown", shiftKey: true })).toBe(
    true,
  );
  fireEvent.compositionStart(draft);
  expect(
    fireEvent.keyDown(draft, { key: "ArrowDown", isComposing: true }),
  ).toBe(true);
  expect(document.activeElement).toBe(draft);
});
it("navigates clarification choices and skips the disabled third row", async () => {
  const f = fixture();
  const user = userEvent.setup();
  render(<App bridge={f.bridge} />);
  const draft = await screen.findByRole("textbox");
  f.update({
    phase: "clarification",
    canInsert: false,
    clarification: { question: "Which?", choices: ["First", "Second"] },
  });
  draft.focus();
  await user.keyboard("{ArrowUp}{ArrowDown}{Enter}");
  await waitFor(() =>
    expect(
      f.calls.some((c) => c.action.type === "choose" && c.action.index === 0),
    ).toBe(true),
  );
  expect(document.activeElement).toBe(draft);
});
