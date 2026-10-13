// v12's `Selector` (src/query/selector.ts), ported to JS over JS `Node`s.
//
// The selector text is parsed by v12's own selector grammar (the cpat text
// below, copied from src/query/selector_parser.ts), built in wasm like any
// grammar; the walk over the selector's tree is v12's, line for line, so
// its quirks come along (DESIGN §21, §22).

import { patterns } from "./template.mjs";
import { generateErrorMessage } from "./error_message.mjs";

let selectorParser = null;

// Built once, the first time a selector is made.
function parser() {
  if (selectorParser == null) {
    selectorParser = patterns`
number = /[+-]?(\\d+(\\.\\d*)?|\\.\\d+)([eE][+-]?\\d+)?/
spaces = /\\s+/
single-quote-string-literal = /'(?:\\\\.|[^'\\\\])*'/
name = /[a-zA-Z_-]+[a-zA-Z0-9_-]*/
comma = /\\s*,\\s*/
wild-card = "*"
equal = "="
not-equal = "!="
starts-with = "^="
ends-with = "$="
contains = "*="
greater-than-or-equal = ">="
less-than-or-equal = "<="
greater-than = ">"
less-than = "<"
operators =      equal |
             not-equal | 
           starts-with | 
             ends-with | 
              contains | 
 greater-than-or-equal | 
    less-than-or-equal | 
          greater-than | 
             less-than
             
attribute-name = name
value = name
attribute-value = single-quote-string-literal | number | value
attribute-selector = "[" + spaces? + attribute-name + spaces? + operators + spaces? + attribute-value + "]"

adjacent = spaces? + "+" + spaces?
after = spaces? + "~" + spaces?
direct-child = spaces? + ">" + spaces?
descendant = spaces

combinators =  adjacent | after | direct-child | descendant
name-selector = name-selector-expression + attribute-selector
name-selector-expression = name-selector | name
node-selector = wild-card | attribute-selector | name-selector-expression
or-selector = (node-selector, comma){2}

selector-expression = expression + combinators + expression
expression = selector-expression | or-selector | node-selector 
`.expression;
  }
  return selectorParser;
}

const combinatorMap = {
  adjacent: true,
  after: true,
  descendant: true,
  "direct-child": true,
};

const operatorMap = {
  equal: true,
  "not-equal": true,
  "starts-with": true,
  "ends-with": true,
  contains: true,
  "greater-than-or-equal": true,
  "less-than-or-equal": true,
  "greater-than": true,
  "less-than": true,
};

export class Selector {
  _selectedNodes;
  _selectorAst;
  _combinator;

  constructor(selector) {
    this._selectedNodes = [];
    this._combinator = null;

    const result = parser().exec(selector);

    if (result.ast == null) {
      const message = generateErrorMessage(parser(), result.cursor);
      throw new Error(`[Invalid Selector] ${message}`);
    }

    this._selectorAst = result.ast;
  }

  find(nodes) {
    this._selectedNodes = nodes;

    const ast = this._selectorAst;
    ast.walkUp(node => {
      this._process(node);
    });

    return this._selectedNodes;
  }

  filter(nodes) {
    if (nodes.length < 1) {
      return [];
    }

    const nodeMap = new Map();
    nodes.forEach(n => {
      nodeMap.set(n, n);
    });

    this._selectedNodes = [nodes[0].findRoot()];

    const ast = this._selectorAst;
    ast.walkUp(node => {
      this._process(node);
    });

    return this._selectedNodes.filter(n => nodeMap.has(n));
  }

  not(nodes) {
    if (nodes.length < 1) {
      return [];
    }

    this._selectedNodes = [nodes[0].findRoot()];

    const ast = this._selectorAst;
    ast.walkUp(node => {
      this._process(node);
    });

    const selectedNodeMap = new Map();
    this._selectedNodes.forEach(n => {
      selectedNodeMap.set(n, n);
    });

    return nodes.filter(n => !selectedNodeMap.has(n));
  }

  parents(nodes) {
    if (nodes.length < 1) {
      return [];
    }

    this._selectedNodes = [nodes[0].findRoot()];

    const ast = this._selectorAst;
    ast.walkUp(node => {
      this._process(node);
    });

    const result = new Set();
    const ancestorMap = new Map();
    this._selectedNodes.forEach(n => {
      ancestorMap.set(n, true);
    });

    nodes.forEach(n => {
      const ancestor = n.findAncestor(a => ancestorMap.has(a));
      if (ancestor != null) {
        result.add(ancestor);
      }
    });

    return Array.from(result);
  }

  _process(ast) {
    const nodeName = ast.name;

    if (nodeName === "wild-card") {
      this._selectedNodes = this._processWildCard();
    } else if (nodeName === "or-selector") {
      this._selectedNodes = this._processOrSelector(ast);
    } else if (
      nodeName === "name-selector" ||
      (nodeName === "name" &&
        (ast.parent == null || ast.parent.name === "selector-expression"))
    ) {
      this._selectedNodes = this._processNameSelector(ast);
    } else if (
      nodeName === "attribute-selector" &&
      (ast.parent == null || ast.parent.name === "selector-expression")
    ) {
      this._selectedNodes = this._processAttributeSelector(ast);
    } else if (combinatorMap[nodeName]) {
      this._combinator = nodeName;
    } else if (nodeName === "selector-expression") {
      this._combinator = null;
    }
  }

  _processWildCard() {
    return this._selectedNodes.flatMap(n => {
      return this._selectWithCombinator(n, () => true);
    });
  }

  _processOrSelector(ast) {
    const selectorNodes = ast.children.filter(n => n.name !== "comma");
    const set = new Set();

    const selectors = selectorNodes.map(n => new Selector(n.toString()));

    selectors
      .flatMap(s => {
        return s.find(this._selectedNodes.slice());
      })
      .forEach(node => {
        set.add(node);
      });

    return Array.from(set);
  }

  _processNameSelector(ast) {
    if (ast.children.length > 1) {
      return this._selectedNodes.flatMap(n => {
        const name = ast.children[0].value;

        return this._selectWithCombinator(n, node => {
          return node.name === name && this._isAttributeMatch(node, ast);
        });
      });
    } else {
      return this._selectedNodes.flatMap(n => {
        return this._selectWithCombinator(n, node => node.name === ast.value);
      });
    }
  }

  _processAttributeSelector(ast) {
    return this._selectedNodes.flatMap(n => {
      return this._selectWithCombinator(n, node => {
        return this._isAttributeMatch(node, ast);
      });
    });
  }

  _selectWithCombinator(node, predicate) {
    if (this._combinator === "adjacent") {
      const sibling = node.nextSibling();
      if (sibling == null) {
        return [];
      }

      if (predicate(sibling)) {
        return [sibling];
      } else {
        return [];
      }
    } else if (this._combinator === "after") {
      const parent = node.parent;

      if (parent == null) {
        return [];
      }

      const index = parent.findChildIndex(node);
      const after = parent.children.slice(index + 1);
      return after.filter(predicate);
    } else if (this._combinator === "direct-child") {
      return node.children.filter(predicate);
    } else if (this._combinator === "descendant" || this._combinator == null) {
      return node.findAll(predicate);
    } else {
      return [];
    }
  }

  _isAttributeMatch(node, ast) {
    const name = this._getAttributeName(ast);
    const operator = this._getAttributeOperator(ast);
    const value = this._getAttributeValue(ast);
    // Attribute selectors name a Node property at runtime ("[value='4']") and
    // compare it with JS's own operators, exactly as v12 does.
    const anyNode = node;

    if (anyNode[name] == null) {
      return false;
    }

    if (operator === "equal") {
      return anyNode[name] === value;
    } else if (operator === "not-equal") {
      return anyNode[name] !== value;
    } else if (operator === "starts-with") {
      return anyNode[name].toString().startsWith(value);
    } else if (operator === "ends-with") {
      return anyNode[name].toString().endsWith(value);
    } else if (operator === "contains") {
      return anyNode[name].toString().includes(value);
    } else if (operator === "greater-than-or-equal") {
      return anyNode[name] >= value;
    } else if (operator === "less-than-or-equal") {
      return anyNode[name] <= value;
    } else if (operator === "greater-than") {
      return anyNode[name] > value;
    } else if (operator === "less-than") {
      return anyNode[name] < value;
    }

    return false;
  }

  _getAttributeName(ast) {
    return ast.find(n => n.name === "attribute-name").value;
  }

  _getAttributeValue(ast) {
    let valueNode = ast.find(n => n.name === "single-quote-string-literal");

    if (valueNode != null) {
      return valueNode.value.slice(1, -1);
    } else {
      valueNode = ast.find(n => n.name === "value");
    }

    if (valueNode != null) {
      return valueNode.value;
    } else {
      valueNode = ast.find(n => n.name === "number");
    }

    return valueNode.value;
  }

  _getAttributeOperator(ast) {
    return ast.find(n => operatorMap[n.name]).name;
  }
}
