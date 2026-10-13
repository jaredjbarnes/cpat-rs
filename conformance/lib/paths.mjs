// Where the harness finds things. The two sibling checkouts default to the
// layout this repo is developed in; CPAT_V12 and AGENT_HARNESS override.
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

export const conformanceDir = dirname(dirname(fileURLToPath(import.meta.url)));
export const repoDir = dirname(conformanceDir);
export const corpusDir = join(conformanceDir, "corpus");
export const workDir = join(repoDir, "target", "conformance");
export const wasmDir = join(repoDir, "crates", "cpat-wasm");
export const wasmApi = join(wasmDir, "js", "index.mjs");
export const nativeBin = join(repoDir, "target", "release", "cpat-conformance");

export const v12Dir = resolve(process.env.CPAT_V12 ?? join(repoDir, "..", "clarity-pattern-parser-v12"));
export const v12Api = join(v12Dir, "src", "index.ts");
export const agentHarnessDir = resolve(
  process.env.AGENT_HARNESS ?? join(repoDir, "..", "agent-harness", "packages", "harness")
);
