import { describe, expect, it } from "vitest";
import { STEPS } from "./constants";

describe("wizard steps", () => {
  it("keeps source preview before target selection", () => {
    expect(STEPS.map((step) => step.id)).toEqual([
      "source",
      "commits",
      "preview",
      "target",
      "migrate",
    ]);
    expect(STEPS.map((step) => step.num)).toEqual([1, 2, 3, 4, 5]);
  });
});
