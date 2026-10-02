import { describe, expect, it } from "vitest";
import { NO_REMINDER, leadOf, moveTaskTimes, reminderForDue, separateReminder, taskAnchorTime } from "./due";

describe("due time helpers", () => {
  it("computes the reminder from due time and lead", () => {
    expect(reminderForDue("2026-10-09T18:00", 0)).toBe("2026-10-09T18:00");
    expect(reminderForDue("2026-10-09T18:00", 60)).toBe("2026-10-09T17:00");
    expect(reminderForDue("2026-10-09T00:10", 1440)).toBe("2026-10-08T00:10");
    expect(reminderForDue("2026-10-09T18:00", NO_REMINDER)).toBe("");
    expect(reminderForDue("", 15)).toBe("");
  });

  it("recovers the lead from stored times", () => {
    expect(leadOf("2026-10-09T18:00", "2026-10-09T17:45")).toBe(15);
    expect(leadOf("2026-10-09T18:00", "2026-10-08T18:00")).toBe(1440);
    expect(leadOf("2026-10-09T18:00", "")).toBe(NO_REMINDER);
    expect(leadOf("2026-10-09T18:00", "2026-10-09T16:20")).toBe("custom");
    expect(leadOf("", "2026-10-09T16:20")).toBe("custom");
  });

  it("anchors tasks on due time first", () => {
    expect(taskAnchorTime({ dueAt: "2026-10-09T18:00:00", reminderTime: "2026-10-09T17:00:00" })).toBe("2026-10-09T18:00:00");
    expect(taskAnchorTime({ dueAt: null, reminderTime: "2026-10-09T17:00:00" })).toBe("2026-10-09T17:00:00");
    expect(taskAnchorTime({})).toBeNull();
    expect(separateReminder({ dueAt: "2026-10-09T18:00:00", reminderTime: "2026-10-09T17:00:00" })).toBe("2026-10-09T17:00:00");
    expect(separateReminder({ dueAt: "2026-10-09T18:00:00", reminderTime: "2026-10-09T18:00:00" })).toBeNull();
    expect(separateReminder({ reminderTime: "2026-10-09T18:00:00" })).toBeNull();
  });
});

describe("moveTaskTimes", () => {
  it("moves the due time and keeps the lead", () => {
    expect(
      moveTaskTimes({ dueAt: "2026-10-09T18:00:00", reminderTime: "2026-10-09T17:00:00" }, "2026-10-12T18:00:00")
    ).toEqual({ dueAt: "2026-10-12T18:00:00", reminderTime: "2026-10-12T17:00:00" });
    expect(moveTaskTimes({ dueAt: "2026-10-09T18:00:00", reminderTime: null }, "2026-10-12T18:00:00")).toEqual({
      dueAt: "2026-10-12T18:00:00",
      reminderTime: null
    });
    expect(moveTaskTimes({ reminderTime: "2026-10-09T17:00:00" }, "2026-10-12T17:00:00")).toEqual({
      reminderTime: "2026-10-12T17:00:00"
    });
  });
});
