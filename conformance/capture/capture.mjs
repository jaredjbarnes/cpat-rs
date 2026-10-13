// Recaptures the corpus: runs agent-harness's vitest suite and v12's own
// grammar tests with clarity-pattern-parser swapped for a logging copy of
// v12, then assembles what they ran into conformance/corpus. Neither repo is
// changed: the configs, shim and logs live in the --scratch directory.
//
//   node conformance/capture/capture.mjs --scratch /some/tmp/dir
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { agentHarnessDir, v12Api, v12Dir } from "../lib/paths.mjs";
import { assemble } from "./assemble.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const scratchArg = process.argv.indexOf("--scratch");
if (scratchArg < 0) {
  console.error("usage: node conformance/capture/capture.mjs --scratch <dir>");
  process.exit(2);
}
const scratch = process.argv[scratchArg + 1];
mkdirSync(scratch, { recursive: true });

const fill = (file, values) => {
  let text = readFileSync(join(here, file), "utf8");
  for (const [key, value] of Object.entries(values)) text = text.replaceAll(key, value);
  writeFileSync(join(scratch, file), text);
};

copyFileSync(join(here, "capture.ts"), join(scratch, "capture.ts"));
const harnessSrc = join(agentHarnessDir, "src");
fill("harness_shim.ts", { __V12_API__: v12Api.replace(/\.ts$/, ""), __HARNESS_SRC__: harnessSrc });
fill("jest_setup.ts", { __V12_SRC__: join(v12Dir, "src"), __V12_DIR__: v12Dir });

writeFileSync(
  join(scratch, "vitest.harness.config.mjs"),
  `export default {
  resolve: { conditions: ["source"], alias: [{ find: /^clarity-pattern-parser$/, replacement: ${JSON.stringify(join(scratch, "harness_shim.ts"))} }] },
  ssr: { resolve: { conditions: ["source"] } },
  test: {
    root: ${JSON.stringify(agentHarnessDir)},
    environment: "node",
    globals: true,
    server: { deps: { inline: [/^@subqdocs\\//, /clarity-pattern-parser/] } },
  },
};
`
);
writeFileSync(
  join(scratch, "jest.v12.config.js"),
  `module.exports = {
  rootDir: ${JSON.stringify(v12Dir)},
  preset: "ts-jest",
  testEnvironment: "node",
  roots: ["<rootDir>/src"],
  testMatch: ["**/typescript.test.ts", "**/Grammar.test.ts", "**/AutoComplete.test.ts", "**/Cut.test.ts"],
  transform: { "^.+\\\\.(ts|tsx)$": ["ts-jest", { diagnostics: false }] },
  setupFilesAfterEnv: [${JSON.stringify(join(scratch, "jest_setup.ts"))}],
};
`
);

const harnessOut = join(scratch, "out-harness");
const v12Out = join(scratch, "out-v12");
rmSync(harnessOut, { recursive: true, force: true });
rmSync(v12Out, { recursive: true, force: true });

// Failing tests don't stop the capture: v12 itself fails two agent-harness
// tests (MIGRATION-v12.md, "agent-harness"), and what they ran still counts.
const run = (cmd, args, cwd, out) => {
  try {
    execFileSync(cmd, args, { cwd, stdio: "inherit", env: { ...process.env, CAPTURE_OUT: out } });
  } catch (error) {
    console.error(`(${cmd} ${args[0]} exited ${error.status}; keeping what was captured)`);
  }
};
run("npx", ["vitest", "run", "--config", join(scratch, "vitest.harness.config.mjs")], agentHarnessDir, harnessOut);
run("npx", ["jest", "--config", join(scratch, "jest.v12.config.js")], v12Dir, v12Out);

assemble([harnessOut, v12Out]);
