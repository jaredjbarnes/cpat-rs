// The accessor tests of clarity-pattern-parser v12's
// src/patterns/Expression.test.ts, run against js/index.mjs with their
// assertions as written but one (DESIGN §22), then v12's answers for the
// same grammars (printed by running v12's src with tsx) pinned by name.
import { describe, test, expect } from "./testing/expect.mjs";
import {
  Expression,
  Literal,
  Optional,
  Reference,
  Regex,
  RightAssociated,
  Sequence,
  patterns,
} from "./index.mjs";

function createExpressionPattern() {
  const spaces = new Regex("spaces", "\\s+");
  spaces.setTokens([" "]);

  const optionalSpaces = new Optional("optional-spaces", spaces);

  const variables = new Expression("variables", [
    new Literal("a", "a"),
    new Literal("b", "b"),
    new Literal("c", "c"),
    new Literal("d", "d"),
    new Literal("e", "e"),
  ]);

  const mulDivOperator = new Expression("mult-div", [new Literal("mult", " * "), new Literal("div", " / ")]);

  const addSubOperator = new Expression("add-sub", [new Literal("add", " + "), new Literal("sub", " - ")]);

  const boolOperator = new Expression("and-or", [new Literal("and", " && "), new Literal("or", " || ")]);

  const group = new Sequence("group", [
    new Literal("open-paren", "("),
    optionalSpaces,
    new Reference("expression"),
    optionalSpaces,
    new Literal("close-paren", ")"),
  ]);

  const ternary = new Sequence("ternary", [
    new Reference("expression"),
    optionalSpaces,
    new Literal("question-mark", "?"),
    optionalSpaces,
    new Reference("expression"),
    optionalSpaces,
    new Literal("colon", ":"),
    optionalSpaces,
    new Reference("expression"),
  ]);

  const multDivExpression = new Sequence("mult-div-expression", [
    new Reference("expression"),
    mulDivOperator,
    new Reference("expression"),
  ]);

  const addSubExpression = new Sequence("add-sub-expression", [
    new Reference("expression"),
    addSubOperator,
    new Reference("expression"),
  ]);

  const boolExpression = new Sequence("bool-expression", [
    new Reference("expression"),
    boolOperator,
    new Reference("expression"),
  ]);

  const expression = new Expression("expression", [
    multDivExpression,
    addSubExpression,
    boolExpression,
    new RightAssociated(ternary),
    group,
    variables,
  ]);

  return expression;
}

function accessorExpression() {
  const negate = new Sequence("negate", [new Literal("minus", "-"), new Reference("expression")]);
  const bang = new Sequence("factorial", [new Reference("expression"), new Literal("bang", "!")]);
  const add = new Sequence("add", [new Reference("expression"), new Literal("plus", "+"), new Reference("expression")]);
  const variable = new Literal("variable", "a");

  return new Expression("expression", [add, negate, bang, variable]);
}

describe("Expression Pattern", () => {
  test("Accessor: atomPatterns returns atom patterns", () => {
    const expression = createExpressionPattern();
    expression.build();

    // The expression has group and variables as atoms
    expect(expression.atomPatterns.length).toBeGreaterThan(0);
  });

  test("Accessor: infixPatterns, prefixPatterns, postfixPatterns", () => {
    const expression = accessorExpression();
    expression.build();

    expect(expression.prefixPatterns.length).toBe(1);
    expect(expression.postfixPatterns.length).toBe(1);
    expect(expression.infixPatterns.length).toBe(1);
    expect(expression.atomPatterns.length).toBe(1);
  });

  test("Accessor: binaryPatterns is deprecated alias for infixPatterns", () => {
    const expression = createExpressionPattern();
    expression.build();

    expect(expression.binaryPatterns).toBe(expression.infixPatterns);
  });

  test("Accessor: originalPatterns returns the original list", () => {
    const a = new Literal("a", "a");
    const b = new Literal("b", "b");
    const expression = new Expression("expression", [a, b]);

    // v12: `toContain(a)`, the very object given. Composites here take
    // copies (DESIGN §22), so the originals are equal patterns, not `a`.
    expect(expression.originalPatterns.some(p => p.isEqual(a))).toBe(true);
    expect(expression.originalPatterns.some(p => p.isEqual(b))).toBe(true);
    expect(expression.originalPatterns.length).toBe(2);
  });
});

// type:name of each accessor's patterns, as v12 answers after build().
const names = patterns => patterns.map(p => `${p.type}:${p.name}`);
const roles = expression => ({
  atom: names(expression.atomPatterns),
  prefix: names(expression.prefixPatterns),
  postfix: names(expression.postfixPatterns),
  infix: names(expression.infixPatterns),
  original: names(expression.originalPatterns),
  children: names(expression.children),
});

describe("Expression accessors, compared with v12", () => {
  test("the accessor grammar", () => {
    expect(roles(accessorExpression())).toEqual({
      atom: ["literal:variable"],
      prefix: ["sequence:negate-prefix"],
      postfix: ["sequence:factorial-postfix"],
      infix: ["sequence:add-delimiter"],
      original: ["sequence:add", "sequence:negate", "sequence:factorial", "literal:variable"],
      children: ["sequence:add-delimiter", "sequence:negate-prefix", "sequence:factorial-postfix", "literal:variable"],
    });
  });

  test("createExpressionPattern", () => {
    const answer = roles(createExpressionPattern());
    expect(answer.infix).toEqual([
      "sequence:mult-div-expression-delimiter",
      "sequence:add-sub-expression-delimiter",
      "sequence:bool-expression-delimiter",
      "sequence:ternary-delimiter",
    ]);
    expect(answer.atom).toEqual(["sequence:group", "expression:variables"]);
    expect([answer.prefix, answer.postfix]).toEqual([[], []]);
  });

  test("a grammar's expression, its operators reached through references", () => {
    const { expr } = patterns`
      num = /\\d+/
      neg = "-" + expr
      add = expr + "+" + expr
      inc = expr + "++"
      expr = add | neg | inc | num
    `;
    // Each rule is a context over its pattern, as in v12.
    const expression = expr.children[0];
    expect(expression.type).toBe("expression");
    expect(roles(expression)).toEqual({
      atom: ["regex:num"],
      prefix: ["sequence:neg-prefix"],
      postfix: ["sequence:inc-postfix"],
      infix: ["sequence:add-delimiter"],
      original: ["sequence:add", "sequence:neg", "sequence:inc", "regex:num"],
      children: ["sequence:add-delimiter", "sequence:neg-prefix", "sequence:inc-postfix", "regex:num"],
    });
  });

  test("the accessors' patterns are the expression's children, and answer as places", () => {
    const expression = accessorExpression();
    const [add] = expression.infixPatterns;

    expect(add).toBe(expression.children[0]);
    expect(add.parent.name).toBe("expression");
    expect(add.children.map(c => c.name)).toEqual(["plus"]);
    // `readonly Pattern[]` in v12's types; frozen here, so a push can't
    // desynchronise the cache from the children.
    expect(Object.isFrozen(expression.infixPatterns)).toBe(true);
  });

  test("a right-associated original reads as the pattern it marks", () => {
    // v12 answers `right-associated:` (an empty name) for the marker itself;
    // a marker is no place here (DESIGN §16, §22).
    const ternary = new Sequence("ternary", [
      new Reference("expression"),
      new Literal("q", "?"),
      new Reference("expression"),
      new Literal("c", ":"),
      new Reference("expression"),
    ]);
    const expression = new Expression("expression", [new RightAssociated(ternary), new Literal("x", "x")]);

    expect(names(expression.infixPatterns)).toEqual(["sequence:ternary-delimiter"]);
    expect(names(expression.originalPatterns)).toEqual(["sequence:ternary", "literal:x"]);
  });

  test("an ordered choice's alternatives are all atoms", () => {
    const expression = new Expression("abc", [new Literal("a", "a"), new Literal("b", "b")]);
    expect(names(expression.atomPatterns)).toEqual(["literal:a", "literal:b"]);
    expect(expression.infixPatterns).toEqual([]);
  });
});
