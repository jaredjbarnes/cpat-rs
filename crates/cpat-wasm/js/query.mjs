// v12's `Query` and `$` (src/query/query.ts), ported to JS over JS
// `Node`s: jQuery-style selection and editing of a parse tree, line for
// line (DESIGN §21, §22).

import { Node } from "./node.mjs";
import { Selector } from "./selector.mjs";

export class Query {
  _context;
  _prevQuery;

  constructor(context, prevQuery = null) {
    this._context = context;
    this._prevQuery = prevQuery;
  }

  toArray() {
    return this._context.slice();
  }

  // Modifiers
  append(visitor) {
    this._context.forEach(n => {
      const parent = n.parent;

      if (parent == null) {
        return;
      }
      const newNode = visitor(n);
      n.appendChild(newNode);
    });

    return this;
  }

  prepend(visitor) {
    this._context.forEach(n => {
      const parent = n.parent;

      if (parent == null) {
        return;
      }
      const newNode = visitor(n);
      n.insertBefore(newNode, n.children[0]);
    });

    return this;
  }

  after(visitor) {
    this._context.forEach(n => {
      const parent = n.parent;

      if (parent == null) {
        return;
      }
      const index = parent.findChildIndex(n);
      const newNode = visitor(n);

      parent.spliceChildren(index + 1, 0, newNode);
    });

    return this;
  }

  before(visitor) {
    this._context.forEach(n => {
      const parent = n.parent;

      if (parent == null) {
        return;
      }
      const index = parent.findChildIndex(n);
      const newNode = visitor(n);

      parent.spliceChildren(index, 0, newNode);
    });

    return this;
  }

  replaceWith(visitor) {
    this._context.forEach(n => {
      const newNode = visitor(n);
      n.replaceWith(newNode);
    });

    return this;
  }

  compact() {
    this._context.forEach(n => {
      n.compact();
    });

    return this;
  }

  setValue(value) {
    this.replaceWith(n => {
      return Node.createValueNode(n.type, n.name, value);
    });

    return this;
  }

  normalize() {
    const first = this._context[0];

    if (first != null) {
      first.findRoot().normalize();
    }
  }

  remove() {
    this._context.forEach(n => {
      n.remove();
    });

    return this;
  }

  // Filters from the currently matched nodes
  slice(start, end) {
    return new Query(this._context.slice(start, end));
  }

  filter(selectorString) {
    const selector = new Selector(selectorString);
    const newContext = selector.filter(this._context);

    return new Query(newContext, this);
  }

  // Selects out of all descedants of currently matched nodes
  find(selectorString) {
    const selector = new Selector(selectorString);
    const newContext = selector.find(this._context);

    return new Query(newContext, this);
  }

  // Remove nodes from the set of matched nodes.
  not(selectorString) {
    const selector = new Selector(selectorString);
    const newContext = selector.not(this._context);

    return new Query(newContext, this);
  }

  // Select the parent of currently matched nodes
  parent() {
    const parents = this._context.map(n => n.parent);
    const result = new Set();
    parents.forEach(n => {
      if (n != null) {
        result.add(n);
      }
    });

    return new Query(Array.from(result), this);
  }

  // Select the ancestors of currently matched nodes
  parents(selectorString) {
    const selector = new Selector(selectorString);
    const newContext = selector.parents(this._context);

    const result = new Set();
    newContext.forEach(n => {
      if (n != null) {
        result.add(n);
      }
    });

    return new Query(Array.from(result), this);
  }

  first() {
    return new Query(this._context.slice(0, 1), this);
  }

  last() {
    return new Query(this._context.slice(-1), this);
  }

  // Pop query stack
  end() {
    if (this._prevQuery) {
      return this._prevQuery;
    }
    return this;
  }

  length() {
    return this._context.length;
  }
}

export function $(root, selector) {
  if (selector == null) {
    return new Query([root]);
  } else {
    return new Query([root]).find(selector);
  }
}
