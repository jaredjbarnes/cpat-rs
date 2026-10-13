// clarity-pattern-parser for agent-harness's own vitest run: v12's src, with
// every plain grammar build and its exec/test/AutoComplete calls logged.
// capture.mjs fills in the two paths and writes this into a scratch dir.
import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";
import * as v12 from "__V12_API__";
import { install } from "./capture";

const root = "__HARNESS_SRC__";
const files = execSync(`find ${root} -name "*.cpat"`).toString().trim().split("\n");
const byText = new Map(files.map(f => [readFileSync(f, "utf8"), f.slice(root.length + 1)]));

install(
  v12.Grammar,
  v12.AutoComplete,
  "agent-harness",
  text => byText.get(text) ?? null,
  () => {
    const state = (globalThis as any).expect?.getState?.();
    return state?.testPath ? `${state.testPath.slice(root.length + 1)} :: ${state.currentTestName ?? ""}` : "";
  }
);

export * from "__V12_API__";
