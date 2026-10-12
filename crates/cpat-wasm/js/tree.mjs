// A read-only view over an ExecResult's flat arrays.
//
// The tree crosses the wasm boundary once, as typed arrays; every property
// below reads those arrays, so walking the tree never calls back into wasm.
// Positions are UTF-16 offsets, so `value` is a plain `text.slice`.

const STRIDE = 7;
// Indexed by the kind codes in src/exec_result.rs.
const KIND = ["literal", "regex", "sequence", "custom", "infinite-repeat", "finite-repeat"];
const NO_PARENT = 0xffffffff;

export class Tree {
  constructor(text, result) {
    this._text = text;
    this._nodes = result.nodes;
    this._children = result.children;
    this._names = result.names;
  }

  get root() {
    return this._nodes.length === 0 ? null : new Node(this, 0);
  }
}

export class Node {
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
    return parent === NO_PARENT ? null : new Node(this._tree, parent);
  }

  get children() {
    const at = this._field(5);
    const len = this._field(6);
    const ids = this._tree._children;
    const children = [];
    for (let i = at; i < at + len; i++) {
      children.push(new Node(this._tree, ids[i]));
    }
    return children;
  }

  get isLeaf() {
    return this._field(6) === 0;
  }

  /** First match, children before their parent. */
  find(predicate) {
    for (const child of this.children) {
      const found = child.find(predicate);
      if (found != null) return found;
    }
    return predicate(this) ? this : null;
  }

  findAll(predicate, found = []) {
    for (const child of this.children) child.findAll(predicate, found);
    if (predicate(this)) found.push(this);
    return found;
  }
}
