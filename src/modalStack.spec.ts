import { describe, expect, it } from "vitest";
import { closeTopModal, hasOpenModal, pushModal } from "./modalStack";

describe("modalStack", () => {
  it("closes only the topmost modal", () => {
    const closed: string[] = [];
    const releaseA = pushModal(() => closed.push("a"));
    const releaseB = pushModal(() => closed.push("b"));
    expect(closeTopModal()).toBe(true);
    expect(closed).toEqual(["b"]);
    releaseB();
    expect(closeTopModal()).toBe(true);
    expect(closed).toEqual(["b", "a"]);
    releaseA();
    expect(hasOpenModal()).toBe(false);
    expect(closeTopModal()).toBe(false);
  });
});
