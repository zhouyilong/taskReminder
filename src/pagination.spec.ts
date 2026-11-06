import { describe, expect, it } from "vitest";
import { clampPage } from "./pagination";

describe("clampPage", () => {
  it("接受范围内的页码", () => {
    expect(clampPage("3", 5)).toBe(3);
    expect(clampPage(" 5 ", 5)).toBe(5);
    expect(clampPage(2, 5)).toBe(2);
  });

  it("超出范围时收回到首页或末页", () => {
    expect(clampPage("0", 5)).toBe(1);
    expect(clampPage("-3", 5)).toBe(1);
    expect(clampPage("99", 5)).toBe(5);
    expect(clampPage("2.7", 5)).toBe(2);
    expect(clampPage("4", 0)).toBe(1);
  });

  it("不是数字时返回 null", () => {
    expect(clampPage("", 5)).toBeNull();
    expect(clampPage("abc", 5)).toBeNull();
    expect(clampPage("1e3", 5)).toBeNull();
    expect(clampPage(undefined, 5)).toBeNull();
  });
});
