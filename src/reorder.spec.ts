import { describe, expect, it } from "vitest";
import { ORDER_STEP, planReorder } from "./reorder";

const list = (...orders: Array<number | null>) => orders.map((sortOrder, index) => ({ id: `t${index}`, sortOrder }));

describe("planReorder", () => {
  it("moves one row to the midpoint of its new neighbours", () => {
    expect(planReorder(list(1024, 2048, 3072), "t2", 1)).toEqual([{ id: "t2", sortOrder: 1536 }]);
    expect(planReorder(list(1024, 2048, 3072), "t0", 3)).toEqual([{ id: "t0", sortOrder: 3072 + ORDER_STEP }]);
    expect(planReorder(list(1024, 2048, 3072), "t2", 0)).toEqual([{ id: "t2", sortOrder: 0 }]);
  });

  it("ignores drops that keep the order", () => {
    expect(planReorder(list(1, 2, 3), "t1", 1)).toEqual([]);
    expect(planReorder(list(1, 2, 3), "t1", 2)).toEqual([]);
    expect(planReorder(list(1, 2, 3), "missing", 0)).toEqual([]);
  });

  it("renumbers when rows have no position yet or the gap is exhausted", () => {
    expect(planReorder(list(null, null, null), "t2", 0)).toEqual([
      { id: "t2", sortOrder: 1024 },
      { id: "t0", sortOrder: 2048 },
      { id: "t1", sortOrder: 3072 }
    ]);
    const tight = planReorder(list(1, 1 + 1e-7, 5), "t2", 1);
    expect(tight.map(item => item.id)).toEqual(["t0", "t2", "t1"]);
  });
});
