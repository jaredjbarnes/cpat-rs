// clarity-pattern-parser's API for Node, over pkg-node. Ready on import.
// Browsers and bundlers use ./web.mjs (the same API over pkg-web, after an
// async init).
import "./load_pkg_node.mjs";

export * from "./api.mjs";
