import { describe, expect, it } from "vitest";
import { nextSoundState } from "./notificationSound";

const item = (recordId: string, sound = true) => ({ recordId, sound });

describe("nextSoundState", () => {
  it("plays once for newly arrived reminders", () => {
    const first = nextSoundState(new Set(), [item("a"), item("b")]);
    expect(first.play).toBe(true);
    // 同一批再次推送（例如稍后提醒了其中一条后的剩余队列）：不再响。
    const again = nextSoundState(first.seen, [item("b")]);
    expect(again.play).toBe(false);
    expect([...again.seen]).toEqual(["b"]);
  });

  it("plays when a new reminder joins an open popup", () => {
    const state = nextSoundState(new Set(["a"]), [item("a"), item("c")]);
    expect(state.play).toBe(true);
  });

  it("stays silent for reminders queued with sound off or during quiet hours", () => {
    expect(nextSoundState(new Set(), [item("a", false)]).play).toBe(false);
    expect(nextSoundState(new Set(), [{ recordId: "legacy" }]).play).toBe(false);
    expect(nextSoundState(new Set(), []).play).toBe(false);
  });
});
