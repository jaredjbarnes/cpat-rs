// v12's editable `Node` (src/ast/Node.ts), ported to plain JS.
//
// The API and its quirks are v12's: `walkUp` stops visiting siblings after a
// `false` but still visits every ancestor (so `find` lets the outermost
// matching ancestor of the first match win), `walkUp`/`walkDown` read a
// node's children before visiting it and `walkBreadthFirst` after, ids come
// from one process-wide counter, and `normalize` clamps a last index at 0.
//
// What differs is how deep trees are walked: no method here recurses
// (DESIGN §7e). An operator chain builds a tree as deep as the input is
// long, and v12's recursive walks throw `RangeError` on it; these keep
// their own stacks and visit in exactly v12's order.

function defaultVisitor(node) {
  return node;
}

let idIndex = 0;

export class Node {
  get id() {
    return this._id;
  }

  get type() {
    return this._type;
  }

  get name() {
    return this._name;
  }

  get value() {
    return this.toString();
  }

  get firstIndex() {
    return this._firstIndex;
  }

  get lastIndex() {
    return this._lastIndex;
  }

  get startIndex() {
    return this._firstIndex;
  }

  get endIndex() {
    return this._lastIndex + 1;
  }

  get parent() {
    return this._parent;
  }

  get children() {
    return this._children;
  }

  get hasChildren() {
    return this._children.length > 0;
  }

  get isLeaf() {
    return !this.hasChildren;
  }

  constructor(type, name, firstIndex, lastIndex, children = [], value = "") {
    this._id = String(idIndex++);
    this._type = type;
    this._name = name;
    this._firstIndex = firstIndex;
    this._lastIndex = lastIndex;
    this._parent = null;
    this._children = children;
    this._value = value;

    this._children.forEach(c => {
      c._parent = this;
    });
  }

  removeChild(node) {
    const index = this._children.indexOf(node);

    if (index > -1) {
      this._children.splice(index, 1);
      node._parent = null;
    }
  }

  findChildIndex(node) {
    return this._children.indexOf(node);
  }

  spliceChildren(index, deleteCount, ...items) {
    const removedItems = this._children.splice(index, deleteCount, ...items);

    removedItems.forEach(i => {
      i._parent = null;
    });
    items.forEach(i => {
      i._parent = this;
    });

    return removedItems;
  }

  removeAllChildren() {
    this.spliceChildren(0, this._children.length);
  }

  replaceChild(newNode, referenceNode) {
    const index = this.findChildIndex(referenceNode);

    if (index > -1) {
      this.spliceChildren(index, 1, newNode);
      newNode._parent = this;
      referenceNode._parent = null;
    }
  }

  replaceWith(newNode) {
    if (this._parent != null) {
      this._parent.replaceChild(newNode, this);
    }
  }

  insertBefore(newNode, referenceNode) {
    newNode._parent = this;

    if (referenceNode == null) {
      this._children.push(newNode);
      return;
    }

    const index = this.findChildIndex(referenceNode);

    if (index > -1) {
      this._children.splice(index, 0, newNode);
    }
  }

  appendChild(newNode) {
    this.append(newNode);
  }

  append(...nodes) {
    nodes.forEach(newNode => {
      newNode._parent = this;
      this._children.push(newNode);
    });
  }

  nextSibling() {
    if (this._parent == null) {
      return null;
    }

    const children = this._parent._children;
    const index = children.indexOf(this);

    if (index > -1 && index < children.length - 1) {
      return children[index + 1];
    }

    return null;
  }

  previousSibling() {
    if (this._parent == null) {
      return null;
    }

    const children = this._parent._children;
    const index = children.indexOf(this);

    if (index > -1 && index > 0) {
      return children[index - 1];
    }

    return null;
  }

  find(predicate, breadthFirst = false) {
    let match = null;
    const visit = n => {
      if (predicate(n)) {
        match = n;
        return false;
      }
    };

    if (breadthFirst) {
      this.walkBreadthFirst(visit);
    } else {
      this.walkUp(visit);
    }

    return match;
  }

  findAll(predicate, breadthFirst = false) {
    const matches = [];
    const visit = n => {
      if (predicate(n)) {
        matches.push(n);
      }
    };

    if (breadthFirst) {
      this.walkBreadthFirst(visit);
    } else {
      this.walkUp(visit);
    }

    return matches;
  }

  findRoot() {
    let node = this;

    while (node.parent != null) {
      node = node.parent;
    }
    return node;
  }

  findAncestor(predicate) {
    let parent = this._parent;

    while (parent != null) {
      if (predicate(parent)) {
        return parent;
      }

      parent = parent._parent;
    }

    return null;
  }

  // v12: `children.slice().every(c => c.walkUp(cb))`, then
  // `(cb(this) ?? true) && result`. Each frame is a node with the copy of
  // its children it took on entry and whether every child so far said yes.
  walkUp(callback) {
    const stack = [{ node: this, children: this._children.slice(), next: 0, ok: true }];
    let result = true;

    while (stack.length > 0) {
      const frame = stack[stack.length - 1];

      if (frame.ok && frame.next < frame.children.length) {
        const child = frame.children[frame.next++];
        stack.push({ node: child, children: child._children.slice(), next: 0, ok: true });
        continue;
      }

      stack.pop();
      result = (callback(frame.node) ?? true) && frame.ok;

      if (stack.length > 0) {
        stack[stack.length - 1].ok = Boolean(result);
      }
    }

    return result;
  }

  // v12: `(cb(this) ?? true) && children.slice().every(c => c.walkDown(cb))`,
  // the copy taken before the visit.
  walkDown(callback) {
    const enter = node => {
      const children = node._children.slice();
      return { visited: callback(node) ?? true, children, next: 0 };
    };

    const first = enter(this);
    if (!first.visited) {
      return first.visited;
    }

    const stack = [first];
    while (stack.length > 0) {
      const frame = stack[stack.length - 1];

      if (frame.next >= frame.children.length) {
        stack.pop();
        continue;
      }

      const child = enter(frame.children[frame.next++]);
      if (!child.visited) {
        // `every` stops at the first falsy child, and so does each
        // ancestor's, all the way up.
        return false;
      }
      stack.push(child);
    }

    return true;
  }

  walkBreadthFirst(callback) {
    const queue = [this];

    for (let at = 0; at < queue.length; at++) {
      const current = queue[at];
      if (callback(current) === false) {
        return false;
      }
      queue.push(...current.children);
    }

    return true;
  }

  // v12 transforms the children first (each from a copy of the list taken
  // on entry), puts the results back, then visits.
  transform(visitors) {
    const visit = node => {
      const visitor = visitors[node.name] == null ? defaultVisitor : visitors[node.name];
      return visitor(node);
    };
    const stack = [{ node: this, children: this._children.slice(), results: [] }];
    let result = null;

    while (stack.length > 0) {
      const frame = stack[stack.length - 1];

      if (frame.results.length < frame.children.length) {
        const child = frame.children[frame.results.length];
        stack.push({ node: child, children: child._children.slice(), results: [] });
        continue;
      }

      stack.pop();
      frame.node.removeAllChildren();
      frame.node.append(...frame.results);
      result = visit(frame.node);

      if (stack.length > 0) {
        stack[stack.length - 1].results.push(result);
      }
    }

    return result;
  }

  flatten() {
    const nodes = [];

    this.walkDown(node => {
      if (!node.hasChildren) {
        nodes.push(node);
      }
    });

    return nodes;
  }

  compact() {
    const value = this.toString();
    this.removeAllChildren();
    this._value = value;
  }

  remove() {
    if (this._parent != null) {
      this._parent.removeChild(this);
    }
  }

  // Children are cloned before their parent, as v12's argument list
  // evaluates them, so ids are handed out in the same order.
  clone() {
    const stack = [{ node: this, clones: [] }];
    let result = null;

    while (stack.length > 0) {
      const frame = stack[stack.length - 1];

      if (frame.clones.length < frame.node._children.length) {
        stack.push({ node: frame.node._children[frame.clones.length], clones: [] });
        continue;
      }

      stack.pop();
      const n = frame.node;
      result = new Node(n._type, n._name, n._firstIndex, n._lastIndex, frame.clones, n._value);

      if (stack.length > 0) {
        stack[stack.length - 1].clones.push(result);
      }
    }

    return result;
  }

  normalize(startIndex = this._firstIndex) {
    const stack = [{ node: this, start: startIndex, running: startIndex, length: 0, next: 0 }];
    let length = 0;

    while (stack.length > 0) {
      const frame = stack[stack.length - 1];
      const children = frame.node.children;

      if (children.length > 0 && frame.next < children.length) {
        const child = children[frame.next++];
        stack.push({ node: child, start: frame.running, running: frame.running, length: 0, next: 0 });
        continue;
      }

      stack.pop();
      if (children.length === 0) {
        frame.length = frame.node._value.length;
      }
      frame.node._firstIndex = frame.start;
      frame.node._lastIndex = Math.max(frame.start + frame.length - 1, 0);
      length = frame.length;

      if (stack.length > 0) {
        const parent = stack[stack.length - 1];
        parent.running += length;
        parent.length += length;
      }
    }

    return length;
  }

  // A parent's text is its leaves' text in order, which is what v12's
  // recursive join of joins comes to.
  toString() {
    if (this._children.length === 0) {
      return this._value;
    }

    let text = "";
    const stack = [this];
    while (stack.length > 0) {
      const node = stack.pop();
      if (node._children.length === 0) {
        text += node._value;
      } else {
        for (let i = node._children.length - 1; i >= 0; i--) {
          stack.push(node._children[i]);
        }
      }
    }
    return text;
  }

  toCycleFreeObject() {
    const stack = [{ node: this, objects: [] }];
    let result = null;

    while (stack.length > 0) {
      const frame = stack[stack.length - 1];
      const n = frame.node;

      if (frame.objects.length < n._children.length) {
        stack.push({ node: n._children[frame.objects.length], objects: [] });
        continue;
      }

      stack.pop();
      result = {
        id: n._id,
        type: n._type,
        name: n._name,
        value: n._children.length === 0 ? n._value : frame.objects.map(o => o.value).join(""),
        startIndex: n.startIndex,
        endIndex: n.endIndex,
        children: frame.objects,
      };

      if (stack.length > 0) {
        stack[stack.length - 1].objects.push(result);
      }
    }

    return result;
  }

  toJson(space) {
    return JSON.stringify(this.toCycleFreeObject(), null, space);
  }

  // v12 also compares `value` at every level; equal children make equal
  // joined values, so comparing it at the leaves gives the same answer
  // without joining each subtree's text again.
  isEqual(node) {
    const pairs = [[this, node]];

    while (pairs.length > 0) {
      const [a, b] = pairs.pop();

      if (
        b._type !== a._type ||
        b._name !== a._name ||
        b._firstIndex !== a._firstIndex ||
        b._lastIndex !== a._lastIndex ||
        a._children.length !== b._children.length
      ) {
        return false;
      }

      if (a._children.length === 0) {
        if (a._value !== b._value) {
          return false;
        }
        continue;
      }

      for (let i = a._children.length - 1; i >= 0; i--) {
        pairs.push([a._children[i], b._children[i]]);
      }
    }

    return true;
  }

  static createValueNode(type, name, value = "") {
    return new Node(type, name, 0, 0, [], value);
  }

  static createNode(type, name, children = []) {
    const value = children.map(c => c.toString()).join("");
    return new Node(type, name, 0, 0, children, value);
  }
}
