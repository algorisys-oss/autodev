import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@solidjs/testing-library";
import type * as ipcTypes from "../lib/ipc";
import { SettingsPanel } from "./settings-panel";
import { bellEnabled, setBellEnabled } from "../lib/bell";

vi.mock("../lib/ipc", () => ({
  getSettings: vi.fn(),
  setSettings: vi.fn(),
}));
import * as ipc from "../lib/ipc";

const mocked = ipc as unknown as {
  getSettings: ReturnType<typeof vi.fn>;
  setSettings: ReturnType<typeof vi.fn>;
};

const base: ipcTypes.AppSettings = {
  theme: "system",
  defaultEffort: "high",
  transcribeCommand: null,
  screenshotCommand: null,
  browserCommand: null,
};

describe("settings panel", () => {
  beforeEach(() => {
    mocked.getSettings.mockReset();
    mocked.setSettings.mockReset();
    setBellEnabled(false);
  });

  it("loads settings, edits a command, and saves — persisting blank fields as null", async () => {
    mocked.getSettings.mockResolvedValue({ ...base, screenshotCommand: "grim {file}" });
    mocked.setSettings.mockResolvedValue(base);

    const { getByPlaceholderText, getByText, findByText } = render(() => (
      <SettingsPanel onClose={() => {}} />
    ));

    // Existing value is shown once loaded.
    const shot = (await waitFor(() =>
      getByPlaceholderText(/grim \{file\}/),
    )) as HTMLInputElement;
    expect(shot.value).toBe("grim {file}");

    // Set the transcribe command; leave browser blank.
    const transcribe = getByPlaceholderText(/whisper-cli/) as HTMLInputElement;
    fireEvent.input(transcribe, { target: { value: "my-transcriber {file}" } });

    fireEvent.click(getByText("Save"));
    await findByText("saved");

    expect(mocked.setSettings).toHaveBeenCalledTimes(1);
    const payload = mocked.setSettings.mock.calls[0][0] as ipcTypes.AppSettings;
    expect(payload.transcribeCommand).toBe("my-transcriber {file}");
    expect(payload.screenshotCommand).toBe("grim {file}");
    expect(payload.browserCommand).toBeNull();
  });

  it("toggles the bell — live for this session, and persisted on save", async () => {
    mocked.getSettings.mockResolvedValue({ ...base, bellOnComplete: false });
    mocked.setSettings.mockResolvedValue(base);

    const { getByLabelText, getByText, findByText } = render(() => (
      <SettingsPanel onClose={() => {}} />
    ));

    const bell = (await waitFor(() => getByLabelText(/Ring a bell/))) as HTMLInputElement;
    expect(bell.checked).toBe(false);
    expect(bellEnabled()).toBe(false);

    fireEvent.click(bell);
    expect(bellEnabled()).toBe(true); // applies immediately, before Save

    fireEvent.click(getByText("Save"));
    await findByText("saved");
    const payload = mocked.setSettings.mock.calls[0][0] as ipcTypes.AppSettings;
    expect(payload.bellOnComplete).toBe(true);
  });

  it("closes via the Close button", async () => {
    mocked.getSettings.mockResolvedValue(base);
    const onClose = vi.fn();
    const { getByText, findByText } = render(() => <SettingsPanel onClose={onClose} />);
    await findByText("Settings");
    fireEvent.click(getByText("Close"));
    expect(onClose).toHaveBeenCalled();
  });
});
