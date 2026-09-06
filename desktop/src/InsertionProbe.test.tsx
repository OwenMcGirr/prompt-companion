// @vitest-environment jsdom
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import InsertionProbe, { type ProbeStatus } from "./InsertionProbe";
afterEach(cleanup);
const base: ProbeStatus = {
  available: true,
  enabled: false,
  armed: false,
  manual_codex: false,
  manual_codex_available: true,
  token: 4,
  destination: null,
  message: "Ready",
};
it("starts off and sends a distinct manual Codex opt-in", async () => {
  const call = vi.fn().mockResolvedValue(base);
  render(<InsertionProbe text="TEST " call={call} />);
  const checkbox = await screen.findByRole("checkbox", {
    name: /Include Codex/,
  });
  expect((checkbox as HTMLInputElement).checked).toBe(false);
  expect(
    (
      screen.getByRole("button", {
        name: "Insert natively",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  fireEvent.click(checkbox);
  await waitFor(() =>
    expect(call).toHaveBeenLastCalledWith({ kind: "manualCodex", value: true }),
  );
});
it("suppresses rapid clicks while one attempt is pending and includes capture token", async () => {
  let finish!: (status: ProbeStatus) => void;
  const call = vi
    .fn()
    .mockResolvedValueOnce({ ...base, enabled: true, destination: "Codex" })
    .mockImplementationOnce(
      () =>
        new Promise<ProbeStatus>((resolve) => {
          finish = resolve;
        }),
    );
  render(<InsertionProbe text="TEST " call={call} />);
  const button = await screen.findByRole("button", { name: "Insert natively" });
  fireEvent.click(button);
  fireEvent.click(button);
  expect(call).toHaveBeenCalledTimes(2);
  expect(call).toHaveBeenLastCalledWith({
    kind: "native",
    text: "TEST ",
    token: 4,
  });
  finish({ ...base, enabled: true, message: "Uncertain outcome" });
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toBe("Uncertain outcome"),
  );
  expect((button as HTMLButtonElement).disabled).toBe(true);
  expect(call).toHaveBeenCalledTimes(2);
});
it("clears the local target after IPC failure without retry", async () => {
  const call = vi
    .fn()
    .mockResolvedValueOnce({ ...base, enabled: true, destination: "Codex" })
    .mockRejectedValueOnce(new Error("Disconnected"));
  render(<InsertionProbe text="TEST " call={call} />);
  fireEvent.click(
    await screen.findByRole("button", { name: "Paste with clipboard" }),
  );
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain(
      "No retry was made",
    ),
  );
  expect(
    (
      screen.getByRole("button", {
        name: "Insert natively",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(call).toHaveBeenCalledTimes(2);
});
it("hides the Codex option on unsupported platforms", async () => {
  render(
    <InsertionProbe
      text="TEST"
      call={async () => ({ ...base, manual_codex_available: false })}
    />,
  );
  await screen.findByRole("heading", { name: "Manual insertion test" });
  expect(screen.queryByRole("checkbox", { name: /Include Codex/ })).toBeNull();
});
