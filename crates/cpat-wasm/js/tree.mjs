// A read-only view over an ExecResult's flat arrays.
//
// The tree crosses the wasm boundary once, as typed arrays; every property
// below reads those arrays, so walking the tree never calls back into wasm.
// Positions are UTF-16 offsets, so `value` is a plain `text.slice`.
// `toNode()` turns the view into v12's editable `Node` (./node.mjs).

import { Node } from "./node.mjs";

const STRIDE = 7;
// Indexed by the kind codes in src/exec_result.rs.
const KIND = [
  "literal",
  "regex",
  "sequence",
  "custom",
  "infinite-repeat",
  "finite-repeat",
  "expression",
  "take-until",
  "block",
  "block-content",
];
const NO_PARENT = 0xffffffff;

export class Tree {
  constructor(text, result) {
    this._text = text;
    this._nodes = result.nodes;
    this._children = result.children;
    this._names = result.names;
  }

  get root() {
    return this._nodes.length === 0 ? null : new NodeView(this, 0);
  }
}

export class NodeView {
  constructor(tree, index) {
    this._tree = tree;
    this._at = index * STRIDE;
  }

  _field(offset) {
    return this._tree._nodes[this._at + offset];
  }

  get type() {
    return KIND[this._field(0)];
  }

  get name() {
    return this._tree._names[this._field(1)];
  }

  get startIndex() {
    return this._field(2);
  }

  get endIndex() {
    return this._field(3);
  }

  get firstIndex() {
    return this.startIndex;
  }

  /** Inclusive, as in clarity-pattern-parser. */
  get lastIndex() {
    return this.endIndex - 1;
  }

  get value() {
    return this._tree._text.slice(this.startIndex, this.endIndex);
  }

  get parent() {
    const parent = this._field(4);
    return parent === NO_PARENT ? null : new NodeView(this._tree, parent);
  }

  get children() {
    const at = this._field(5);
    const len = this._field(6);
    const ids = this._tree._children;
    const children = [];
    for (let i = at; i < at + len; i++) {
      children.push(new NodeView(this._tree, ids[i]));
    }
    return children;
  }

  get isLeaf() {
    return this._field(6) === 0;
  }

  /** First match, children before their parent. */
  find(predicate) {
    for (const node of this._postOrder()) {
      if (predicate(node)) return node;
    }
    return null;
  }

  findAll(predicate) {
    return [...this._postOrder()].filter(predicate);
  }

  // Children before their parent, with its own stack: an operator chain
  // makes a tree as deep as the input is long, too deep to recurse over.
  *_postOrder() {
    const pending = [[this, 0]];
    while (pending.length > 0) {
      const top = pending[pending.length - 1];
      const [node, done] = top;
      if (done === node._field(6)) {
        pending.pop();
        yield node;
      } else {
        top[1] = done + 1;
        const child = node._tree._children[node._field(5) + done];
        pending.push([new NodeView(node._tree, child), 0]);
      }
    }
  }

  /**
   * This node and everything under it as v12's editable `Node`: TS-shaped
   * `firstIndex`/`lastIndex` (inclusive) and `startIndex`/`endIndex`, in
   * UTF-16 offsets. A leaf keeps its text; a parent's own value is "", as
   * a v12 parse node's is, so its `value` is its children's text.
   *
   * Nodes are made from the last index back: in the flat layout every
   * descendant comes after its ancestor, so each child exists before its
   * parent needs it, with no recursion however deep the tree.
   */
  toNode() {
    const tree = this._tree;
    const first = this._at / STRIDE;
    const made = new Map();
    let last = first;
    // The subtree is the run of indices from this node to its last
    // descendant: follow last children down.
    for (let at = first * STRIDE; tree._nodes[at + 6] > 0; at = last * STRIDE) {
      last = tree._children[tree._nodes[at + 5] + tree._nodes[at + 6] - 1];
    }

    for (let index = last; index >= first; index--) {
      const at = index * STRIDE;
      const start = tree._nodes[at + 2];
      const end = tree._nodes[at + 3];
      const childrenAt = tree._nodes[at + 5];
      const childCount = tree._nodes[at + 6];
      const children = [];
      for (let i = childrenAt; i < childrenAt + childCount; i++) {
        children.push(made.get(tree._children[i]));
        made.delete(tree._children[i]);
      }
      const value = childCount === 0 ? tree._text.slice(start, end) : "";
      const type = KIND[tree._nodes[at]];
      const name = tree._names[tree._nodes[at + 1]];
      made.set(index, new Node(type, name, start, end - 1, children, value));
    }

    return made.get(first);
  }
}
