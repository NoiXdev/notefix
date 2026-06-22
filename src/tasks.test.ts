import { describe, it, expect } from "vitest";
import { countTasks } from "./tasks";

describe("countTasks", () => {
  it("returns 0/0 with no task items", () => {
    expect(countTasks("<p>hello</p>")).toEqual({ done: 0, total: 0 });
  });

  it("counts done and total", () => {
    const html =
      '<ul><li data-checked="true">a</li><li data-checked="false">b</li><li data-checked="true">c</li></ul>';
    expect(countTasks(html)).toEqual({ done: 2, total: 3 });
  });
});
