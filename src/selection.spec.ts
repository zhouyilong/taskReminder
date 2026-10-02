import { describe, expect, it } from "vitest";
import { emptySelection, pruneSelection, selectRange, toggleAll, toggleSelected } from "./selection";

const ordered = ["a", "b", "c", "d", "e"];
const ids = (state: { ids: Set<string> }) => [...state.ids].sort();

describe("selection", () => {
  it("toggles single rows and remembers the anchor", () => {
    let state = toggleSelected(emptySelection(), "b");
    expect(ids(state)).toEqual(["b"]);
    expect(state.anchor).toBe("b");
    state = toggleSelected(state, "b");
    expect(ids(state)).toEqual([]);
  });

  it("selects a range from the anchor in either direction", () => {
    let state = toggleSelected(emptySelection(), "b");
    state = selectRange(state, ordered, "d");
    expect(ids(state)).toEqual(["b", "c", "d"]);
    expect(state.anchor).toBe("b");
    state = selectRange(state, ordered, "a");
    expect(ids(state)).toEqual(["a", "b", "c", "d"]);
  });

  it("falls back to toggling without an anchor", () => {
    expect(ids(selectRange(emptySelection(), ordered, "c"))).toEqual(["c"]);
  });

  it("toggles all rows of the page", () => {
    let state = toggleAll(toggleSelected(emptySelection(), "z"), ["a", "b"]);
    expect(ids(state)).toEqual(["a", "b", "z"]);
    state = toggleAll(state, ["a", "b"]);
    expect(ids(state)).toEqual(["z"]);
    expect(ids(toggleAll(emptySelection(), []))).toEqual([]);
  });

  it("prunes rows that are no longer visible", () => {
    const state = toggleSelected(toggleSelected(emptySelection(), "a"), "c");
    const pruned = pruneSelection(state, ["a", "b"]);
    expect(ids(pruned)).toEqual(["a"]);
    expect(pruned.anchor).toBeNull();
    expect(pruneSelection(pruned, ["a"])).toBe(pruned);
  });
});
