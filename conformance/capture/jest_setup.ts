// jest setup for v12's own tests: log every plain grammar they build, and
// what they run through it. setupFilesAfterEnv shares the test file's module
// registry, so patching the prototypes reaches the test's Grammar.
// capture.mjs fills in the paths and writes this into a scratch dir.
import { Grammar } from "__V12_SRC__/grammar/Grammar";
import { AutoComplete } from "__V12_SRC__/intellisense/AutoComplete";
import { install } from "./capture";

const root = "__V12_DIR__/";
install(Grammar, AutoComplete, "v12-tests", () => null, () => {
  const state = (expect as any).getState();
  return `${(state.testPath ?? "").replace(root, "")} :: ${state.currentTestName ?? ""}`;
});
