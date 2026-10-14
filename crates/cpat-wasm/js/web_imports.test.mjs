// The web entry must load where Node's built-ins don't exist. Walk every
// module js/web.mjs reaches (static and dynamic imports, pkg-web's glue
// included) and check that none names a `node:` module or uses a Node-only
// global. Reverting js/wasm.mjs to require pkg-node would fail here.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const SPECIFIER = /(?:\bfrom\s*|\bimport\s*\(?\s*)["']([^"']+)["']/g;
const NODE_ONLY = /\b(?:require\s*\(|process\.|Buffer\.|__dirname|__filename|createRequire)\b/;

// Comments may mention Node or show an import; only code counts.
const withoutComments = (source) =>
  source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");

function reachable(entry) {
  const seen = new Map();
  const queue = [entry];
  while (queue.length > 0) {
    const url = queue.pop();
    if (seen.has(url.href)) continue;
    const source = withoutComments(readFileSync(url, "utf8"));
    const specifiers = [...source.matchAll(SPECIFIER)].map((m) => m[1]);
    seen.set(url.href, { source, specifiers });
    for (const s of specifiers) {
      if (s.startsWith(".")) queue.push(new URL(s, url));
    }
  }
  return seen;
}

const modules = reachable(new URL("./web.mjs", import.meta.url));

test("the walk reaches the API and pkg-web's glue", () => {
  const names = [...modules.keys()].map((href) => href.split("/").slice(-2).join("/"));
  for (const expected of ["js/api.mjs", "js/patterns.mjs", "js/wasm.mjs", "pkg-web/cpat_wasm.js"]) {
    assert.ok(names.includes(expected), `${expected} not reached: ${names.join(", ")}`);
  }
  assert.ok(!names.includes("js/load_pkg_node.mjs"));
});

test("no module the web entry reaches imports a Node built-in or a bare package", () => {
  for (const [href, { specifiers }] of modules) {
    for (const s of specifiers) {
      assert.ok(s.startsWith("."), `${href} imports ${s}`);
    }
  }
});

test("no module the web entry reaches uses a Node-only global", () => {
  for (const [href, { source }] of modules) {
    assert.doesNotMatch(source, NODE_ONLY, href);
  }
});
