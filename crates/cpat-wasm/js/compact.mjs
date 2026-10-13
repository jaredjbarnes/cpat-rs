// v12's `compact` and `remove`: by node name, breadth first.

export function compact(node, nodeMap) {
  node.walkBreadthFirst(n => {
    if (nodeMap[n.name]) {
      n.compact();
    }
  });

  return node;
}

export function remove(node, nodeMap) {
  node.walkBreadthFirst(n => {
    if (nodeMap[n.name]) {
      n.remove();
    }
  });

  return node;
}
