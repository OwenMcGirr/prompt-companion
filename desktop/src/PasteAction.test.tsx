// @vitest-environment jsdom
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import PasteAction from "./PasteAction";
const status = {
  available: true,
  enabled: true,
  click_available: true,
  click_armed: false,
  armed: false,
  manual_codex: true,
  manual_codex_available: true,
  token: 1,
  destination: null,
  message: "Ready",
};
afterEach(cleanup);
const props = {
  text: "hello",
  revision: 7,
  taskId: "task",
  ready: true,
  onCopy: vi.fn(),
};
it("arms clipboard paste with authoritative revision and prevents rapid duplicate requests", async () => {
  let finish!: (v: typeof status) => void;
  const call = vi
    .fn()
    .mockResolvedValueOnce(status)
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
  render(<PasteAction {...props} call={call} />);
  const button = await screen.findByRole("button", {
    name: "Paste on next field click",
  });
  fireEvent.click(button);
  fireEvent.click(button);
  expect(call).toHaveBeenCalledTimes(2);
  expect(call).toHaveBeenLastCalledWith({
    kind: "armPaste",
    text: "hello",
    revision: 7,
    task_id: "task",
  });
  finish({ ...status, click_armed: true });
  expect(
    await screen.findByRole("button", { name: "Cancel waiting paste" }),
  ).toBeTruthy();
  call.mockResolvedValue(status);
  fireEvent.click(screen.getByRole("button", { name: "Cancel waiting paste" }));
  await waitFor(() =>
    expect(call).toHaveBeenLastCalledWith({ kind: "cancel" }),
  );
  expect(props.onCopy).not.toHaveBeenCalled();
});
it("disables paste while draft changes are unacknowledged", async () => {
  render(
    <PasteAction
      {...props}
      ready={false}
      call={vi.fn().mockResolvedValue(status)}
    />,
  );
  expect(
    (
      (await screen.findByRole("button", {
        name: "Paste on next field click",
      })) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
});
it("shows an uncertain IPC outcome without repeating or copying", async () => {
  const call = vi
    .fn()
    .mockResolvedValueOnce(status)
    .mockRejectedValueOnce(new Error("Disconnected"));
  render(<PasteAction {...props} call={call} />);
  fireEvent.click(
    await screen.findByRole("button", { name: "Paste on next field click" }),
  );
  await screen.findByText(/Could not confirm paste status/);
  expect(call).toHaveBeenCalledTimes(2);
  expect(props.onCopy).not.toHaveBeenCalled();
});
it("retains Copy as the primary action on unsupported platforms", async () => {
  const onCopy = vi.fn();
  render(
    <PasteAction
      {...props}
      onCopy={onCopy}
      call={vi.fn().mockResolvedValue({ ...status, click_available: false })}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "Copy Prompt" }));
  expect(onCopy).toHaveBeenCalledOnce();
});

it("explains that clearing follows the validated paste command", async () => {
  render(<PasteAction {...props} call={vi.fn().mockResolvedValue(status)} />);
  expect(
    await screen.findByText(/Clears after the validated paste command/),
  ).toBeTruthy();
});
