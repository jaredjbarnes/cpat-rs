// clarity-pattern-parser v12's src/intellisense/AutoComplete.test.ts, run
// against js/index.mjs with its assertions as written (jest's `expect`, from
// ./testing/expect.mjs); then grammar-built AutoComplete compared with v12
// on non-ASCII text: options, UTF-16 start indices, segments, errors, the
// suggestion's cursor, the static methods and generateErrorMessage.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, test, expect } from "./testing/expect.mjs";
import * as api from "./index.mjs";

const { Sequence, Literal, Expression, Reference, Regex, Repeat, Optional, AutoComplete, Cursor, patterns } = api;


function generateFlagFromList(flagNames) {
  return flagNames.map(flagName => {
    return new Literal("flag-name", flagName);
  });
}

function generateFlagPattern(flagNames) {
  const singleFlagOption = flagNames.length === 1 && flagNames[0];
  if (singleFlagOption) {
    return new Literal("flag-name", singleFlagOption);
  }

  const flagPattern = new Expression("flags", generateFlagFromList(flagNames));
  return flagPattern;
}

function optionsMatchExpected(resultOptions, expectedOptions) {
  const expectedOptionsPatternNames = expectedOptions.map(e =>
    e.subElements.map(s => s.pattern)
  );
  const resultOptionsPatternNames = resultOptions.map(r =>
    r.suggestionSequence.map(s => s.pattern.name)
  );

  expect(resultOptions.length).toBe(expectedOptions.length);
  resultOptions.forEach((resultOption, index) => {
    expect(resultOption.text).toBe(expectedOptions[index].text);
    expect(resultOption.startIndex).toBe(expectedOptions[index].startIndex);
    expect(resultOption.suggestionSequence.length).toBe(
      expectedOptions[index].subElements.length
    );
    expect(expectedOptionsPatternNames).toEqual(resultOptionsPatternNames);
  });
}

function generateExpression(flagNames) {
  if (flagNames.length === 0) {
    // regex is purposefully impossible to satisfy
    const noValidOptionsRegex = new Regex("[No Valid Options Exist]", "(?=a)^(?!a)");

    // returning a "Repeat" so as to not break current implementations relying on a Repeat return
    const invalidInputExpression = new Repeat(
      "impossible_expression",
      noValidOptionsRegex
    );

    return invalidInputExpression;
  }

  const openParen = new Literal("open-paren", "(");
  const closeParen = new Literal("close-paren", ")");
  const space = new Regex("[space]", "\\s");
  const and = new Literal("sequence-literal", "AND");
  const or = new Literal("options-literal", "OR");
  const not = new Literal("not", "NOT ");
  const booleanOperator = new Expression("booleanOperator", [and, or]);
  const operatorWithSpaces = new Sequence("operator-with-spaces", [
    space,
    booleanOperator,
    space,
  ]);
  const flag = generateFlagPattern(flagNames);
  const group = new Sequence("group", [
    openParen,
    new Reference("flag-expression"),
    closeParen,
  ]);
  const flagOptionsGroup = new Expression("flag-or-group", [flag, group]);
  const expressionBody = new Sequence("flag-body", [
    new Optional("optional-not", not),
    flagOptionsGroup,
  ]);
  const flagExpression = new Repeat("flag-expression", expressionBody, {
    divider: operatorWithSpaces,
    trimDivider: true,
  });
  return flagExpression;
}

describe("AutoComplete", () => {
  test("No Text", () => {
    const name = new Literal("name", "Name");
    const autoComplete = new AutoComplete(name);
    const result = autoComplete.suggestFor("");

    expect(result.options[0].text).toBe("Name");
    expect(result.options[0].startIndex).toBe(0);
    expect(result.errorAtIndex).toBe(0);
    expect(result.isComplete).toBeFalsy();
  });

  test("Full Pattern Match Simple", () => {
    const john = new Literal("john", "John");
    const space = new Literal("space", " ");
    const doe = new Literal("doe", "Doe");
    const smith = new Literal("smith", "Smith");
    const name = new Sequence("name", [
      john,
      space,
      new Expression("last-name", [smith, doe]),
    ]);

    const autoComplete = new AutoComplete(name);
    const result = autoComplete.suggestFor("John Doe");

    expect(result.ast?.value).toBe("John Doe");
    expect(result.options.length).toBe(0);
    expect(result.errorAtIndex).toBeNull();
    expect(result.isComplete).toBeTruthy();
    expect(result.cursor).not.toBeNull();
  });

  test("More Than One Option", () => {
    const john = new Literal("john", "John");
    const space = new Literal("space", " ");
    const doe = new Literal("doe", "Doe");
    const smith = new Literal("smith", "Smith");
    const lastNameOptions = new Expression("last-name", [smith, doe]);
    const name = new Sequence("name", [john, space, lastNameOptions]);

    const autoComplete = new AutoComplete(name);

    const text = "John ";
    const result = autoComplete.suggestFor(text);

    const expectedOptions = [
      {
        text: "Doe",
        startIndex: 5,
        subElements: [
          {
            text: "Doe",
            pattern: "doe",
          },
        ],
      },
      {
        text: "Smith",
        startIndex: 5,
        subElements: [
          {
            text: "Smith",
            pattern: smith.name,
          },
        ],
      },
    ];

    expect(result.ast).toBeNull();
    optionsMatchExpected(result.options, expectedOptions);
    expect(result.errorAtIndex).toBe(text.length);
    expect(result.isComplete).toBeFalsy();
    expect(result.cursor).not.toBeNull();
  });

  test("Option should error at furthest match index", () => {
    const john = new Literal("john", "John");
    const space = new Literal("space", " ");
    const doe = new Literal("doe", "Doe");
    const smith = new Literal("smith", "Smith");
    const lastNameOptions = new Expression("last-name", [smith, doe]);
    const name = new Sequence("name", [john, space, lastNameOptions]);

    const text = "John Smi";
    const autoComplete = new AutoComplete(name);
    const result = autoComplete.suggestFor(text);

    const expectedOptions = [
      {
        text: "th",
        startIndex: 8,
        subElements: [
          {
            text: "Smith",
            pattern: smith.name,
          },
        ],
      },
    ];

    expect(result.ast).toBeNull();
    optionsMatchExpected(result.options, expectedOptions);
    expect(result.errorAtIndex).toBe(text.length - 1);
    expect(result.isComplete).toBeFalsy();
    expect(result.cursor).not.toBeNull();
  });

  test("Root Regex Pattern suggests customTokens", () => {
    const freeTextPattern = new Regex(`free-text`, "[(\\w)\\s]+");

    const customTokensMap = {
      "free-text": ["luke", "leia skywalker", "luke skywalker"],
    };
    const autoComplete = new AutoComplete(freeTextPattern, {
      customTokens: customTokensMap,
    });
    const result = autoComplete.suggestFor("luke");

    const expectedOptions = [
      {
        text: " skywalker",
        startIndex: 4,
        subElements: [
          {
            text: " skywalker",
            pattern: freeTextPattern.name,
          },
        ],
      },
    ];

    expect(result.ast?.value).toBe("luke");
    optionsMatchExpected(result.options, expectedOptions);
    expect(result.errorAtIndex).toBeNull();
    expect(result.isComplete).toBeTruthy();
    expect(result.cursor).not.toBeNull();
  });

  test("Sequence Regex Pattern suggests customTokens", () => {
    const jediLiteral = new Literal("jedi", "jedi ");
    const freeTextPattern = new Regex(`free-text`, "[(\\w)\\s]+");
    const sequence = new Sequence("sequence", [jediLiteral, freeTextPattern]);

    const customTokensMap = {
      "free-text": ["luke", "leia skywalker", "luke skywalker"],
    };
    const autoComplete = new AutoComplete(sequence, {
      customTokens: customTokensMap,
    });
    const result = autoComplete.suggestFor("jedi luke sky");

    const expected = [
      {
        text: "walker",
        startIndex: 13,
        subElements: [
          {
            text: "walker",
            startIndex: 13,
            pattern: freeTextPattern.name,
          },
        ],
      },
    ];

    expect(result.ast?.value).toBe("jedi luke sky");
    optionsMatchExpected(result.options, expected);
    expect(result.errorAtIndex).toBeNull();
    expect(result.isComplete).toBeTruthy();
    expect(result.cursor).not.toBeNull();
  });

  test("Full Pattern Match With Root Repeat", () => {
    const john = new Literal("john", "John");
    const space = new Literal("space", " ");
    const doe = new Literal("doe", "Doe");
    const smith = new Literal("smith", "Smith");
    const name = new Sequence("name", [
      john,
      space,
      new Expression("last-name", [smith, doe]),
    ]);
    const divider = new Regex("divider", "\\s+,\\s+");

    divider.setTokens([", "]);

    const text = "John Doe";
    const repeat = new Repeat("last-names", name, { divider });
    const autoComplete = new AutoComplete(repeat);
    const result = autoComplete.suggestFor(text);
    const expectedOptions = [
      {
        text: ", ",
        startIndex: 8,
        subElements: [
          {
            text: ", ",
            pattern: "divider",
          },
        ],
      },
    ];

    expect(result.ast?.value).toBe(text);
    optionsMatchExpected(result.options, expectedOptions);
    expect(result.errorAtIndex).toBeNull();
    expect(result.isComplete).toBeTruthy();
    expect(result.cursor).not.toBeNull();
  });

  test("Partial Simple", () => {
    const name = new Literal("name", "Name");
    const autoComplete = new AutoComplete(name);
    // Use deprecated suggest for code coverage.
    const result = autoComplete.suggestFor("Na");
    const expectedOptions = [
      {
        text: "me",
        startIndex: 2,
        subElements: [
          {
            text: "me",
            pattern: "name",
          },
        ],
      },
    ];

    expect(result.ast).toBeNull();
    optionsMatchExpected(result.options, expectedOptions);
    expect(result.errorAtIndex).toBe(1);
    expect(result.isComplete).toBeFalsy();
    expect(result.cursor).not.toBeNull();
  });

  test("Partial Match With Bad Characters", () => {
    const name = new Literal("name", "Name");
    const autoComplete = new AutoComplete(name);
    const result = autoComplete.suggestFor("Ni");

    const expectedOptions = [
      {
        text: "ame",
        startIndex: 1,
        subElements: [
          {
            text: "ame",
            pattern: "name",
          },
        ],
      },
    ];

    expect(result.ast).toBeNull();
    optionsMatchExpected(result.options, expectedOptions);
    expect(result.errorAtIndex).toBe(1);
    expect(result.isComplete).toBeFalsy();
    expect(result.cursor).not.toBeNull();
  });

  test("Complete", () => {
    const name = new Literal("name", "Name");
    const autoComplete = new AutoComplete(name);
    const text = "Name";
    const result = autoComplete.suggestFor(text);

    expect(result.ast?.value).toBe(text);
    optionsMatchExpected(result.options, []);
    expect(result.errorAtIndex).toBeNull();
    expect(result.isComplete).toBeTruthy();
    expect(result.cursor).not.toBeNull();
  });

  test("Options AutoComplete on Composing Pattern", () => {
    const autoCompleteOptions = {
      greedyPatternNames: ["space"],
      customTokens: {
        "last-name": ["Sparrow"],
      },
    };

    const jack = new Literal("jack", "Jack");
    const john = new Literal("john", "John");
    const space = new Literal("space", " ");
    const doe = new Literal("doe", "Doe");
    const smith = new Literal("smith", "Smith");
    const firstName = new Expression("first-name", [jack, john]);
    const lastName = new Expression("last-name", [doe, smith]);
    const fullName = new Sequence("full-name", [firstName, space, lastName]);

    const text = "Jack";
    const autoComplete = new AutoComplete(fullName, autoCompleteOptions);
    const result = autoComplete.suggestFor(text);

    const expectedOptions = [
      {
        text: " Doe",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: space.name,
          },
          {
            text: "Doe",
            pattern: doe.name,
          },
        ],
      },
      {
        text: " Smith",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: space.name,
          },
          {
            text: "Smith",
            pattern: smith.name,
          },
        ],
      },
      {
        text: " Sparrow",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: space.name,
          },
          {
            text: "Sparrow",
            pattern: "last-name",
          },
        ],
      },
    ];

    expect(result.ast).toBeNull();
    expect(result.errorAtIndex).toBe(4);
    optionsMatchExpected(result.options, expectedOptions);
  });

  test("Options AutoComplete on Root Pattern", () => {
    const jack = new Literal("first-name", "Jack");
    const john = new Literal("first-name", "John");

    const names = new Expression("names", [jack, john]);
    const divider = new Literal("divider", ", ");
    const repeat = new Repeat("name-list", names, { divider, trimDivider: true });

    const autoCompleteOptions = {
      customTokens: {
        "first-name": ["James"],
      },
      disableDedupe: true,
    };
    const autoComplete = new AutoComplete(repeat, autoCompleteOptions);

    const text = "";
    const results = autoComplete.suggestFor(text);

    const expectedOptions = [
      {
        text: "Jack",
        startIndex: 0,
        subElements: [
          {
            text: "Jack",
            pattern: jack.name,
          },
        ],
      },
      {
        text: "James",
        startIndex: 0,
        subElements: [
          {
            text: "James",
            pattern: "first-name",
          },
        ],
      },
      {
        text: "James",
        startIndex: 0,
        subElements: [
          {
            text: "James",
            pattern: "first-name",
          },
        ],
      },
      {
        text: "John",
        startIndex: 0,
        subElements: [
          {
            text: "John",
            pattern: john.name,
          },
        ],
      },
    ];

    optionsMatchExpected(results.options, expectedOptions);
    // because autoCompleteOptions specifies "last-name" which is shared by two literals, we get two suggestions each mapping to a respective pattern of that name
    expect(results.options[0].suggestionSequence[0].pattern.id).toBe(jack.id);
    expect(results.options[2].suggestionSequence[0].pattern.id).toBe(john.id);
  });

  test("Options AutoComplete On Leaf Pattern", () => {
    const autoCompleteOptions = {
      greedyPatternNames: ["space"],
      customTokens: {
        space: ["  "],
      },
    };

    const jack = new Literal("jack", "Jack");
    const john = new Literal("john", "John");
    const space = new Literal("space", " ");
    const doe = new Literal("doe", "Doe");
    const smith = new Literal("smith", "Smith");
    const firstName = new Expression("first-name", [jack, john]);
    const lastName = new Expression("last-name", [doe, smith]);
    const fullName = new Sequence("full-name", [firstName, space, lastName]);

    const text = "Jack";
    const autoComplete = new AutoComplete(fullName, autoCompleteOptions);
    const results = autoComplete.suggestFor(text);
    const expectedOptions = [
      {
        text: "  Doe",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: space.name,
          },
          {
            text: "Doe",
            pattern: doe.name,
          },
        ],
      },
      {
        text: "  Smith",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: space.name,
          },
          {
            text: "Smith",
            pattern: smith.name,
          },
        ],
      },
      {
        text: " Doe",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: space.name,
          },
          {
            text: "Doe",
            pattern: doe.name,
          },
        ],
      },
      {
        text: " Smith",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: space.name,
          },
          {
            text: "Smith",
            pattern: smith.name,
          },
        ],
      },
    ];

    expect(results.ast).toBeNull();
    expect(results.errorAtIndex).toBe(4);
    optionsMatchExpected(results.options, expectedOptions);
  });

  test("Match On Different Pattern Roots", () => {
    const start = new Literal("start", "John went to");
    const a = new Literal("a", "a bank.");
    const the = new Literal("the", "the store.");

    const first = new Sequence("first", [start, a]);
    const second = new Sequence("second", [start, the]);

    const both = new Expression("both", [first, second]);

    const autoComplete = new AutoComplete(both);

    const text = "John went to a gas station.";
    const result = autoComplete.suggestFor(text);

    const expected = [
      {
        text: "the store.",
        startIndex: 12,
        subElements: [
          {
            text: "the",
            pattern: the.name,
          },
        ],
      },
      {
        text: "a bank.",
        startIndex: 12,
        subElements: [
          {
            text: "a",
            pattern: a.name,
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);
  });

  test("Options on errors because of string ending, with match", () => {
    const large = new Literal("large", "kahnnnnnn");
    const medium = new Literal("medium", "kahnnnnn");
    const small = new Literal("small", "kahn");

    const smalls = new Expression("kahns", [large, medium, small]);

    const autoComplete = new AutoComplete(smalls);
    const result = autoComplete.suggestFor("kahn");

    const expected = [
      {
        text: "nnnnn",
        startIndex: 4,
        subElements: [
          {
            text: "nnnnn",
            pattern: large.name,
          },
        ],
      },
      {
        text: "nnnn",
        startIndex: 4,
        subElements: [
          {
            text: "nnnn",
            pattern: medium.name,
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);

    expect(result.isComplete).toBeTruthy();
  });

  test("Options on errors because of string ending, between matches", () => {
    const large = new Literal("large", "kahnnnnnn");
    const medium = new Literal("medium", "kahnnnnn");
    const small = new Literal("small", "kahn");

    const smalls = new Expression("kahns", [large, medium, small]);

    const autoComplete = new AutoComplete(smalls);
    const result = autoComplete.suggestFor("kahnn");

    const expected = [
      {
        text: "nnnn",
        startIndex: 5,
        subElements: [
          {
            text: "nnnn",
            pattern: large.name,
          },
        ],
      },
      {
        text: "nnn",
        startIndex: 5,
        subElements: [
          {
            text: "nnn",
            pattern: medium.name,
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);
    expect(result.isComplete).toBeFalsy();
  });

  test("Options on errors because of string ending, match middle", () => {
    const large = new Literal("large", "kahnnnnnn");
    const medium = new Literal("medium", "kahnnnnn");
    const small = new Literal("small", "kahn");
    const smalls = new Expression("kahns", [large, medium, small]);

    const autoComplete = new AutoComplete(smalls);
    const result = autoComplete.suggestFor("kahnnnnn");

    const expected = [
      {
        text: "n",
        startIndex: 8,
        subElements: [
          {
            text: "n",
            pattern: large.name,
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);

    expect(result.isComplete).toBeTruthy();
  });

  test("Options on errors because of string ending on a variety, with match", () => {
    const different3 = new Literal("different-3", "kahnnnnnnn3");
    const different21 = new Literal("different-21", "kahnnnnnn21");
    const different22 = new Literal("different-22", "kahnnnnnn22");
    const different2 = new Literal("different-2", "kahnnnnnn2");
    const different1 = new Literal("different", "kahnnnnnn1");
    const large = new Literal("large", "kahnnnnnn");
    const small = new Literal("small", "kahn");
    const medium = new Literal("medium", "kahnnnnn");

    const smalls = new Expression("kahns", [
      different3,
      different21,
      different22,
      different2,
      different1,
      large,
      medium,
      small,
    ]);

    const autoComplete = new AutoComplete(smalls);
    const result = autoComplete.suggestFor("kahnnnnn");

    const expected = [
      {
        text: "nn3",
        startIndex: 8,
        subElements: [
          {
            text: "nn3",
            pattern: different3.name,
          },
        ],
      },
      {
        text: "n21",
        startIndex: 8,
        subElements: [
          {
            text: "n21",
            pattern: different21.name,
          },
        ],
      },
      {
        text: "n22",
        startIndex: 8,
        subElements: [
          {
            text: "n22",
            pattern: different22.name,
          },
        ],
      },
      {
        text: "n2",
        startIndex: 8,
        subElements: [
          {
            text: "n2",
            pattern: different2.name,
          },
        ],
      },
      {
        text: "n1",
        startIndex: 8,
        subElements: [
          {
            text: "n1",
            pattern: different1.name,
          },
        ],
      },
      {
        text: "n",
        startIndex: 8,
        subElements: [
          {
            text: "n",
            pattern: large.name,
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);
    expect(result.isComplete).toBeTruthy();
  });

  test("Options on errors because of string ending on different branches, with match", () => {
    const smalls = new Expression("kahns", [
      new Sequence("kahn-combo-3", [
        new Literal("kah", "kah"),
        new Sequence("partial", [new Literal("n", "n"), new Literal("three", "3")]),
      ]),
      new Sequence("kahn-combo", [new Literal("kahn", "kahn"), new Literal("one", "1")]),
      new Sequence("kahn-combo-2", [
        new Literal("kahn", "kahn"),
        new Literal("two", "2"),
      ]),
      new Literal("small", "kahn"),
    ]);

    const autoComplete = new AutoComplete(smalls);
    const result = autoComplete.suggestFor("kahn");

    const expected = [
      {
        text: "2",
        startIndex: 4,
        subElements: [
          {
            text: "2",
            pattern: "two",
          },
        ],
      },
      {
        text: "1",
        startIndex: 4,
        subElements: [
          {
            text: "1",
            pattern: "one",
          },
        ],
      },
      {
        text: "3",
        startIndex: 4,
        subElements: [
          {
            text: "3",
            pattern: "three",
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);
    expect(result.isComplete).toBeTruthy();
  });

  test("Remove Trailing Divider", () => {
    const repeat = new Repeat("repeat", new Literal("a", "a"), {
      divider: new Literal("pipe", "|"),
      trimDivider: true,
    });
    const autoComplete = new AutoComplete(repeat);
    const result = autoComplete.suggestFor("a|a|");

    const expected = [
      {
        text: "a",
        startIndex: 4,
        subElements: [
          {
            text: "a",
            pattern: "a",
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);
  });

  test("Remove options divider", () => {
    const jediLuke = new Literal(`jedi`, "luke");
    const names = new Expression("names", [jediLuke]);
    const literalA = new Literal("literal-a", "a");
    const literalB = new Literal("literal-b", "b");

    const optionsDivider = new Expression("options-divider", [literalA, literalB]);

    // control to prove the pattern works without trimDivider
    const controlPattern = new Repeat("name-list", names, { divider: optionsDivider });
    const controlAutoComplete = new AutoComplete(controlPattern);
    const controlResult = controlAutoComplete.suggestFor("lukea");
    expect(controlResult.isComplete).toEqual(true);

    const trimPattern = new Repeat("name-list", names, {
      divider: optionsDivider,
      trimDivider: true,
    });
    const trimAutoComplete = new AutoComplete(trimPattern);

    const trimResult = trimAutoComplete.suggestFor("lukea");
    expect(trimResult.isComplete).toEqual(false);
  });

  test("Expect Divider", () => {
    const repeat = new Repeat("repeat", new Literal("a", "a"), {
      divider: new Literal("pipe", "|"),
    });
    const autoComplete = new AutoComplete(repeat);
    const result = autoComplete.suggestFor("a|a");

    const expected = [
      {
        text: "|",
        startIndex: 3,
        subElements: [
          {
            text: "|",
            pattern: "pipe",
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);
  });

  test("Repeat with bad second repeat", () => {
    const repeat = new Repeat("repeat", new Literal("a", "a"), {
      divider: new Literal("pipe", "|"),
      trimDivider: true,
    });
    const autoComplete = new AutoComplete(repeat);
    const result = autoComplete.suggestFor("a|b");

    const expected = [
      {
        text: "a",
        startIndex: 2,
        subElements: [
          {
            text: "a",
            pattern: "a",
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);
  });

  test("Repeat with bad trailing content", () => {
    const flags = ["FlagA", "FlagB", "FlagC"];
    const pattern = generateExpression(flags);
    const result = new AutoComplete(pattern).suggestFor("FlagA AND FlagAlkjhgB");

    expect(result.options).toEqual([]);
    expect(result.ast?.value).toBe("FlagA AND FlagA");
    expect(result.errorAtIndex).toBe(15);
  });

  test("Ordered Options", () => {
    const john = new Literal("john", "John");
    const doe = new Literal("doe", "Doe");
    const jane = new Literal("jane", "Jane");
    const smith = new Literal("smith", "Smith");
    const space = new Literal("space", " ");

    const firstName = new Expression("first-name", [john, jane]);
    const lastName = new Expression("last-name", [doe, smith]);
    const johnJohnson = new Literal("john-johnson", "John Johnson");
    const johnStockton = new Literal("john-stockton", "John Stockton");
    const fullName = new Sequence("full-name", [firstName, space, lastName]);
    const names = new Expression("names", [johnStockton, johnJohnson, fullName]);

    const autoComplete = new AutoComplete(names);
    const results = autoComplete.suggestFor("John ");
    const expected = [
      {
        text: "Doe",
        startIndex: 5,
        subElements: [
          {
            text: "Doe",
            pattern: "doe",
          },
        ],
      },
      {
        text: "Smith",
        startIndex: 5,
        subElements: [
          {
            text: "Smith",
            pattern: "smith",
          },
        ],
      },
      {
        text: "Stockton",
        startIndex: 5,
        subElements: [
          {
            text: "Stockton",
            pattern: "john-stockton",
          },
        ],
      },
      {
        text: "Johnson",
        startIndex: 5,
        subElements: [
          {
            text: "Johnson",
            pattern: "john-johnson",
          },
        ],
      },
    ];

    optionsMatchExpected(results.options, expected);
    expect(results.ast).toBeNull();
    expect(results.errorAtIndex).toBe(5);
  });

  test("Dedup suggestions", () => {
    const branchOne = new Sequence("branch-1", [
      new Literal("space", " "),
      new Literal("A", "A"),
    ]);
    const branchTwo = new Sequence("branch-2", [
      new Literal("space", " "),
      new Literal("B", "B"),
    ]);
    const eitherBranch = new Expression("either-branch", [branchOne, branchTwo]);

    const autoComplete = new AutoComplete(eitherBranch);
    const results = autoComplete.suggestFor("");
    const expected = [
      {
        startIndex: 0,
        text: " ",
        subElements: [
          {
            text: " ",
            pattern: "space",
          },
        ],
      },
    ];

    optionsMatchExpected(results.options, expected);
  });

  test("Multiple Complex Branches", () => {
    const branchOne = new Sequence("branch-1", [
      new Literal("space-1-1", " "),
      new Literal("space-1-2", " "),
      new Expression("branch-1-options", [
        new Literal("AA", "AA"),
        new Literal("AB", "AB"),
        new Literal("BC", "BC"),
      ]),
    ]);
    const branchTwo = new Sequence("branch-2", [
      new Literal("space-2-1", " "),
      new Literal("space-2-2", " "),
      new Expression("branch-2-options", [
        new Literal("BA", "BA"),
        new Literal("BB", "BB"),
      ]),
    ]);
    const eitherBranch = new Expression("either-branch", [branchOne, branchTwo]);

    const autoComplete = new AutoComplete(eitherBranch);
    const results = autoComplete.suggestFor("  B");

    const expected = [
      {
        startIndex: 3,
        text: "A",
        subElements: [
          {
            text: "A",
            pattern: "BA",
          },
        ],
      },
      {
        startIndex: 3,
        text: "B",
        subElements: [
          {
            text: "B",
            pattern: "BB",
          },
        ],
      },
      {
        startIndex: 3,
        text: "C",
        subElements: [
          {
            text: "C",
            pattern: "BC",
          },
        ],
      },
    ];

    optionsMatchExpected(results.options, expected);
  });

  test("Recursion With Or", () => {
    const ref = new Reference("names");
    const names = new Expression("names", [
      ref,
      new Literal("john", "John"),
      new Literal("jane", "Jane"),
    ]);

    const autoComplete = new AutoComplete(names);
    const suggestion = autoComplete.suggestFor("Jo");

    const expected = [
      {
        text: "hn",
        startIndex: 2,
        subElements: [
          {
            text: "hn",
            pattern: "john",
          },
        ],
      },
    ];

    optionsMatchExpected(suggestion.options, expected);

    expect(suggestion.error?.lastIndex).toBe(1);
  });

  test("Recursion With And", () => {
    const firstNames = new Expression("first-names", [
      new Literal("john", "John"),
      new Literal("jane", "Jane"),
    ]);

    const lastNames = new Expression("last-names", [
      new Literal("doe", "Doe"),
      new Literal("smith", "Smith"),
    ]);

    const fullName = new Sequence("full-name", [
      firstNames,
      new Literal("space", " "),
      lastNames,
    ]);

    const ref = new Reference("names");
    const names = new Sequence("names", [fullName, ref, lastNames]);

    const autoComplete = new AutoComplete(names, {
      greedyPatternNames: ["space"],
    });
    const suggestion = autoComplete.suggestFor("John");

    const expected = [
      {
        text: " Doe",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: "space",
          },
          {
            text: "Doe",
            pattern: "doe",
          },
        ],
      },
      {
        text: " Smith",
        startIndex: 4,
        subElements: [
          {
            text: " ",
            pattern: "space",
          },
          {
            text: "Smith",
            pattern: "smith",
          },
        ],
      },
    ];

    optionsMatchExpected(suggestion.options, expected);
  });

  test("Repeat With Options", () => {
    const john = new Literal("john", "John");
    const jane = new Literal("jane", "Jane");
    const names = new Expression("names", [john, jane]);
    const comma = new Regex("comma", "\\s*,\\s*");
    comma.setTokens([", "]);
    const list = new Repeat("names-list", names, {
      divider: comma,
    });

    const autoComplete = new AutoComplete(list);
    const suggestion = autoComplete.suggestFor("John, ");

    // The trailing divider is kept (trimDivider is off), so the text is a
    // complete match and every repeated option is offered after it.
    expect(suggestion.isComplete).toBe(true);
    expect(suggestion.options.map(o => o.text)).toEqual(["Jane", "John"]);
    expect(suggestion.options.every(o => o.startIndex === 6)).toBe(true);
  });

  test("Repeat With Options With Options", () => {
    const john = new Literal("john", "John");
    const jane = new Literal("jane", "Jane");
    const jack = new Literal("jack", "Jack");
    const jill = new Literal("jill", "Jill");
    const otherNames = new Expression("names", [jack, jill]);
    const names = new Expression("names", [john, jane, otherNames]);
    const comma = new Regex("comma", "\\s*,\\s*");
    comma.setTokens([", "]);
    const list = new Repeat("names-list", names, {
      divider: comma,
      trimDivider: true,
    });

    const autoComplete = new AutoComplete(list, { greedyPatternNames: ["comma"] });
    const suggestion = autoComplete.suggestFor("John");

    // "comma" is greedy, so the divider token is merged with the names that
    // could follow it — including those nested in the inner choice.
    expect(suggestion.isComplete).toBe(true);
    expect(suggestion.options.map(o => o.text)).toEqual([
      ", Jack",
      ", Jane",
      ", Jill",
      ", John",
    ]);
    expect(suggestion.options.every(o => o.startIndex === 4)).toBe(true);
  });

  test("Mid sequence suggestion", () => {
    const operator = new Expression("operator", [
      new Literal("==", "=="),
      new Literal("!=", "!="),
    ]);
    const leftOperands = new Expression("left-operands", [
      new Literal("variable-1", "variable1"),
      new Literal("variable-2", "variable2"),
    ]);

    const rightOperands = new Expression("right-operands", [
      new Literal("variable-3", "variable3"),
      new Literal("variable-4", "variable4"),
    ]);

    const optionalSpace = new Optional("optional-sapce", new Regex("space", "\\s+"));
    const statement = new Sequence("statement", [
      leftOperands,
      optionalSpace,
      operator,
      optionalSpace,
      rightOperands,
    ]);

    const autoComplete = new AutoComplete(statement, {
      greedyPatternNames: ["space", "operator"],
      customTokens: { space: [" "] },
    });
    const result = autoComplete.suggestFor("variable1 ==");

    const expected = [
      {
        text: " variable3",
        startIndex: 12,
        subElements: [
          {
            text: " ",
            pattern: "space",
          },
          {
            text: "variable3",
            pattern: "variable-3",
          },
        ],
      },
      {
        text: " variable4",
        startIndex: 12,
        subElements: [
          {
            text: " ",
            pattern: "space",
          },
          {
            text: "variable4",
            pattern: "variable-4",
          },
        ],
      },
      {
        text: "variable3",
        startIndex: 12,
        subElements: [
          {
            text: "variable3",
            pattern: "variable-3",
          },
        ],
      },
      {
        text: "variable4",
        startIndex: 12,
        subElements: [
          {
            text: "variable4",
            pattern: "variable-4",
          },
        ],
      },
    ];

    optionsMatchExpected(result.options, expected);
  });

  test("Calling AutoComplete -> _getAllOptions Does not mutate customTokensMap", () => {
    const jediLuke = new Literal(`jedi`, "luke");
    const names = new Expression("names", [jediLuke]);
    const divider = new Literal("divider", ", ");
    const pattern = new Repeat("name-list", names, { divider, trimDivider: true });

    const customTokensMap = {
      jedi: ["leia"],
    };

    const copiedCustomTokensMap = JSON.parse(
      JSON.stringify(customTokensMap)
    );

    const autoCompleteOptions = {
      greedyPatternNames: ["jedi"],
      customTokens: customTokensMap,
    };
    const autoComplete = new AutoComplete(pattern, autoCompleteOptions);

    // provide a non-empty input to trigger the logic flow that hits _getAllOptions
    autoComplete.suggestFor("l");

    expect(customTokensMap).toEqual(copiedCustomTokensMap);
  });

  test("Suggests entire greedy node, with appended successive nodes", () => {
    const start = new Literal("start", "start");
    const separator = new Literal("separator", "==");

    const aLiteral = new Literal("letter", "A");
    const bLiteral = new Literal("letter", "B");
    const cLiteral = new Literal("letter", "C");
    const abcOptions = new Expression("letters-options", [aLiteral, bLiteral, cLiteral]);

    const fullSequence = new Sequence("full-sequence", [start, separator, abcOptions]);

    const autoCompleteOptions = {
      greedyPatternNames: ["separator"],
    };
    const autoComplete = new AutoComplete(fullSequence, autoCompleteOptions);

    const text = "start";
    const results = autoComplete.suggestFor(text);

    const expectedOptions = [
      {
        text: "==A",
        startIndex: 5,
        subElements: [
          {
            text: "==",
            pattern: separator.name,
          },
          {
            text: "A",
            pattern: aLiteral.name,
          },
        ],
      },
      {
        text: "==B",
        startIndex: 5,
        subElements: [
          {
            text: "==",
            pattern: separator.name,
          },
          {
            text: "B",
            pattern: bLiteral.name,
          },
        ],
      },
      {
        text: "==C",
        startIndex: 5,
        subElements: [
          {
            text: "==",
            pattern: separator.name,
          },
          {
            text: "C",
            pattern: cLiteral.name,
          },
        ],
      },
    ];

    optionsMatchExpected(results.options, expectedOptions);
  });

  test("incomplete greedy text, suggests node completion with appended successive nodes", () => {
    const start = new Literal("start", "start");
    const separator = new Literal("separator", "==");

    const aLiteral = new Literal("letter", "A");
    const bLiteral = new Literal("letter", "B");
    const cLiteral = new Literal("letter", "C");
    const abcOptions = new Expression("letters-options", [aLiteral, bLiteral, cLiteral]);

    const fullSequence = new Sequence("full-sequence", [start, separator, abcOptions]);

    const autoCompleteOptions = {
      greedyPatternNames: ["separator"],
    };
    const autoComplete = new AutoComplete(fullSequence, autoCompleteOptions);

    const text = "start=";
    const results = autoComplete.suggestFor(text);

    const expectedOptions = [
      {
        text: "=A",
        startIndex: 6,
        subElements: [
          {
            text: "=",
            pattern: separator.name,
          },
          {
            text: "A",
            pattern: aLiteral.name,
          },
        ],
      },
      {
        text: "=B",
        startIndex: 6,
        subElements: [
          {
            text: "=",
            pattern: separator.name,
          },
          {
            text: "B",
            pattern: bLiteral.name,
          },
        ],
      },
      {
        text: "=C",
        startIndex: 6,
        subElements: [
          {
            text: "=",
            pattern: separator.name,
          },
          {
            text: "C",
            pattern: cLiteral.name,
          },
        ],
      },
    ];

    optionsMatchExpected(results.options, expectedOptions);
  });

  test("greedy root node", () => {
    const separator = new Literal("separator", "==");

    const aLiteral = new Literal("letter", "A");
    const bLiteral = new Literal("letter", "B");
    const cLiteral = new Literal("letter", "C");
    const abcOptions = new Expression("letters-options", [aLiteral, bLiteral, cLiteral]);

    const fullSequence = new Sequence("full-sequence", [separator, abcOptions]);

    const autoCompleteOptions = {
      greedyPatternNames: ["separator"],
    };
    const autoComplete = new AutoComplete(fullSequence, autoCompleteOptions);

    const text = "";
    const results = autoComplete.suggestFor(text);

    const expectedOptions = [
      {
        text: "==A",
        startIndex: 0,
        subElements: [
          {
            text: "==",
            pattern: separator.name,
          },
          {
            text: "A",
            pattern: aLiteral.name,
          },
        ],
      },
      {
        text: "==B",
        startIndex: 0,
        subElements: [
          {
            text: "==",
            pattern: separator.name,
          },
          {
            text: "B",
            pattern: bLiteral.name,
          },
        ],
      },
      {
        text: "==C",
        startIndex: 0,
        subElements: [
          {
            text: "==",
            pattern: separator.name,
          },
          {
            text: "C",
            pattern: cLiteral.name,
          },
        ],
      },
    ];

    optionsMatchExpected(results.options, expectedOptions);
  });

  test("options return farthest match when encountering options with shared initial literals", () => {
    const startBracket = new Literal("shared-starting-literal", "{");

    const abcLiteral = new Literal("abc", "abc");
    const xyzLiteral = new Literal("xyz", "xyz");

    const sequenceOne = new Sequence("sequence-one", [startBracket, abcLiteral]);
    const sequenceTwo = new Sequence("sequence-two", [startBracket, xyzLiteral]);

    // create options with sequences that have matching initial literals
    const options = new Expression("options", [sequenceOne, sequenceTwo]);

    const autoComplete = new AutoComplete(options);
    const results = autoComplete.suggestFor("{a");

    const expectedOptions = [
      {
        text: "bc",
        startIndex: 2,
        subElements: [{ text: "abc", pattern: abcLiteral.name }],
      },
    ];
    optionsMatchExpected(results.options, expectedOptions);
  });
});

// ─── Compared with v12 ────────────────────────────────────────────────────

// The same cases through v12's src (tsx) gave V12; through cpat they must
// give the same, but for the two deviations `cpatExpects` applies.
function acCases(api) {
  const { patterns, AutoComplete, Cursor, generateErrorMessage } = api;
  const describe = s => ({
    isComplete: s.isComplete,
    options: s.options.map(o => [o.text, o.startIndex, o.suggestionSequence.map(seg => [seg.text, seg.pattern.name, seg.pattern.type])]),
    error: s.error == null ? null : [s.error.startIndex, s.error.lastIndex],
    errorAtIndex: s.errorAtIndex,
    ast: s.ast == null ? null : [s.ast.value, s.ast.startIndex, s.ast.endIndex],
    cursor: {
      isRecording: s.cursor.isRecording,
      hasError: s.cursor.hasError,
      errors: s.cursor.errors.map(e => [e.pattern.name, e.startIndex, e.lastIndex]),
      matched: s.cursor.allMatchedNodes.map(n => [n.name, n.startIndex, n.endIndex]),
    },
  });
  const grammars = {
    greeting: {
      text: `
        name = "你好" | "👍🏽" | "José" | "Jane"
        space = " "
        verb = "likes" | "loves" | "lüfts"
        statement = name + space + verb + space + name
      `,
      root: "statement",
      options: { greedyPatternNames: ["space"] },
      inputs: ["", "你", "你好", "你好 ", "👍🏽 l", "José lü", "José x", "Jane loves 👍🏽", "Jane loves 👍"],
    },
    tokens: {
      text: `
        @tokens([" "])
        space = /\\s+/
        @tokens(["ünïcode", "日本"])
        word = /[^\\s,]+/
        comma = ","
        words = (word + comma? + space?)+
      `,
      root: "words",
      options: { customTokens: { word: ["😀"] } },
      inputs: ["", "日", "日本", "日本 ", "ünï", "😀,"],
    },
    expression: {
      text: `
        num = /\\d+/
        space = /\\s+/
        neg = "-" + expr
        add = expr + space? + "+" + space? + expr
        mul = expr + space? + "*" + space? + expr
        group = "(" + expr + ")"
        expr = mul | add | neg | group | num
      `,
      root: "expr",
      options: {},
      inputs: ["", "1", "1 ", "1 +", "1 + (", "(2", "-"],
    },
    mutual: {
      text: `
        a = b | "c"
        b = a | "d"
      `,
      root: "a",
      options: {},
      inputs: ["", "c", "x"],
    },
    list: {
      text: `
        name = "Ünal" | "Ödön" | "Zoë"
        divider = ", "
        names = (name, divider trim)+
      `,
      root: "names",
      options: { greedyPatternNames: ["divider"] },
      inputs: ["Ünal", "Ünal, ", "Ünal, Zo", "Zoë, x"],
    },
  };
  const out = {};
  for (const [key, g] of Object.entries(grammars)) {
    const built = patterns([g.text]);
    const pattern = built[g.root];
    const ac = new AutoComplete(pattern, g.options);
    out[key] = g.inputs.map(text => ({ text, ...describe(ac.suggestFor(text)) }));
    out[key + "Static"] = describe(AutoComplete.suggestFor(g.inputs[1], pattern, g.options));
    const cursor = new Cursor(g.inputs[2]);
    const s = AutoComplete.suggestForWithCursor(cursor, pattern, g.options);
    out[key + "WithCursor"] = { sameCursor: s.cursor === cursor, ...describe(s) };
    out[key + "Messages"] = g.inputs.map(text => generateErrorMessage(pattern, pattern.exec(text).cursor));
  }
  return out;
}

const V12 = {"greeting":[{"text":"","isComplete":false,"options":[["👍🏽",0,[["👍🏽","👍🏽","literal"]]],["Jane",0,[["Jane","Jane","literal"]]],["José",0,[["José","José","literal"]]],["你好",0,[["你好","你好","literal"]]]],"error":[0,0],"errorAtIndex":0,"ast":null,"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[]}},{"text":"你","isComplete":false,"options":[["好",1,[["你好","你好","literal"]]]],"error":[0,0],"errorAtIndex":0,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["你好",0,0],["👍🏽",0,0],["José",0,0],["Jane",0,0],["name",0,0]],"matched":[]}},{"text":"你好","isComplete":false,"options":[[" likes",2,[[" ","space","literal"],["likes","likes","literal"]]],[" loves",2,[[" ","space","literal"],["loves","loves","literal"]]],[" lüfts",2,[[" ","space","literal"],["lüfts","lüfts","literal"]]]],"error":[2,2],"errorAtIndex":2,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["statement",0,2]],"matched":[["你好",0,2]]}},{"text":"你好 ","isComplete":false,"options":[["likes",3,[["likes","likes","literal"]]],["loves",3,[["loves","loves","literal"]]],["lüfts",3,[["lüfts","lüfts","literal"]]]],"error":[3,3],"errorAtIndex":3,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["statement",0,3]],"matched":[["你好",0,2],["space",2,3]]}},{"text":"👍🏽 l","isComplete":false,"options":[["ikes",6,[["likes","likes","literal"]]],["oves",6,[["loves","loves","literal"]]],["üfts",6,[["lüfts","lüfts","literal"]]]],"error":[5,5],"errorAtIndex":5,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["你好",0,0],["likes",5,5],["loves",5,5],["lüfts",5,5],["verb",5,5]],"matched":[["👍🏽",0,4],["space",4,5]]}},{"text":"José lü","isComplete":false,"options":[["fts",7,[["lüfts","lüfts","literal"]]]],"error":[6,6],"errorAtIndex":6,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["你好",0,0],["👍🏽",0,0],["likes",5,6],["loves",5,6],["lüfts",5,6],["verb",5,5]],"matched":[["José",0,4],["space",4,5]]}},{"text":"José x","isComplete":false,"options":[["likes",5,[["likes","likes","literal"]]],["loves",5,[["loves","loves","literal"]]],["lüfts",5,[["lüfts","lüfts","literal"]]]],"error":[5,5],"errorAtIndex":5,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["你好",0,0],["👍🏽",0,0],["likes",5,5],["loves",5,5],["lüfts",5,5],["verb",5,5]],"matched":[["José",0,4],["space",4,5]]}},{"text":"Jane loves 👍🏽","isComplete":true,"options":[],"error":null,"errorAtIndex":null,"ast":["Jane loves 👍🏽",0,15],"cursor":{"isRecording":true,"hasError":false,"errors":[["你好",0,0],["👍🏽",0,0],["José",0,1],["likes",5,6],["你好",11,11]],"matched":[["Jane",0,4],["space",4,5],["loves",5,10],["space",10,11],["👍🏽",11,15],["statement",0,15]]}},{"text":"Jane loves 👍","isComplete":false,"options":[["🏽",13,[["👍🏽","👍🏽","literal"]]]],"error":[12,12],"errorAtIndex":12,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["你好",0,0],["👍🏽",0,0],["José",0,1],["likes",5,6],["你好",11,11],["👍🏽",11,12],["José",11,11],["Jane",11,11],["name",11,11]],"matched":[["Jane",0,4],["space",4,5],["loves",5,10],["space",10,11]]}}],"greetingStatic":{"isComplete":false,"options":[["好",1,[["你好","你好","literal"]]]],"error":[0,0],"errorAtIndex":0,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["你好",0,0],["👍🏽",0,0],["José",0,0],["Jane",0,0],["name",0,0]],"matched":[]}},"greetingWithCursor":{"sameCursor":true,"isComplete":false,"options":[[" likes",2,[[" ","space","literal"],["likes","likes","literal"]]],[" loves",2,[[" ","space","literal"],["loves","loves","literal"]]],[" lüfts",2,[[" ","space","literal"],["lüfts","lüfts","literal"]]]],"error":[2,2],"errorAtIndex":2,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["statement",0,2]],"matched":[["你好",0,2]]}},"greetingMessages":["Error at line 1, column 1. Hint: 你好, 👍🏽, José, Jane","Error at line 1, column 1. Hint: 你好, 👍🏽, José, Jane","Error at line 1, column 2. Hint: ","Error at line 1, column 3. Hint: likes,loves,lüfts","Error at line 1, column 6. Hint: likes,loves,lüfts","Error at line 1, column 6. Hint: likes,loves,lüfts","Error at line 1, column 6. Hint: likes,loves,lüfts","Error at line 1, column 15. Hint: ","Error at line 1, column 12. Hint: 你好,👍🏽,José,Jane"],"tokens":[{"text":"","isComplete":false,"options":[["😀",0,[["😀","word","regex"]]],["ünïcode",0,[["ünïcode","word","regex"]]],["日本",0,[["日本","word","regex"]]]],"error":[0,0],"errorAtIndex":0,"ast":null,"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[]}},{"text":"日","isComplete":true,"options":[[" ",1,[[" ","space","regex"]]],[",",1,[[",","comma","literal"]]],["😀",1,[["😀","word","regex"]]],["ünïcode",1,[["ünïcode","word","regex"]]],["日本",1,[["日本","word","regex"]]],["本",1,[["日本","word","regex"]]]],"error":null,"errorAtIndex":null,"ast":["日",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["word",0,1],["anonymous-pattern-0",0,1],["words",0,1]]}},{"text":"日本","isComplete":true,"options":[[" ",2,[[" ","space","regex"]]],[",",2,[[",","comma","literal"]]],["😀",2,[["😀","word","regex"]]],["ünïcode",2,[["ünïcode","word","regex"]]],["日本",2,[["日本","word","regex"]]]],"error":null,"errorAtIndex":null,"ast":["日本",0,2],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["word",0,2],["anonymous-pattern-0",0,2],["words",0,2]]}},{"text":"日本 ","isComplete":true,"options":[["😀",3,[["😀","word","regex"]]],["ünïcode",3,[["ünïcode","word","regex"]]],["日本",3,[["日本","word","regex"]]]],"error":null,"errorAtIndex":null,"ast":["日本 ",0,3],"cursor":{"isRecording":true,"hasError":false,"errors":[["comma",2,2]],"matched":[["word",0,2],["space",2,3],["anonymous-pattern-0",0,3],["words",0,3]]}},{"text":"ünï","isComplete":true,"options":[[" ",3,[[" ","space","regex"]]],[",",3,[[",","comma","literal"]]],["😀",3,[["😀","word","regex"]]],["code",3,[["ünïcode","word","regex"]]],["ünïcode",3,[["ünïcode","word","regex"]]],["日本",3,[["日本","word","regex"]]]],"error":null,"errorAtIndex":null,"ast":["ünï",0,3],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["word",0,3],["anonymous-pattern-0",0,3],["words",0,3]]}},{"text":"😀,","isComplete":true,"options":[[" ",3,[[" ","space","regex"]]],["😀",3,[["😀","word","regex"]]],["ünïcode",3,[["ünïcode","word","regex"]]],["日本",3,[["日本","word","regex"]]]],"error":null,"errorAtIndex":null,"ast":["😀,",0,3],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["word",0,2],["comma",2,3],["anonymous-pattern-0",0,3],["words",0,3]]}}],"tokensStatic":{"isComplete":true,"options":[[" ",1,[[" ","space","regex"]]],[",",1,[[",","comma","literal"]]],["😀",1,[["😀","word","regex"]]],["ünïcode",1,[["ünïcode","word","regex"]]],["日本",1,[["日本","word","regex"]]],["本",1,[["日本","word","regex"]]]],"error":null,"errorAtIndex":null,"ast":["日",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["word",0,1],["anonymous-pattern-0",0,1],["words",0,1]]}},"tokensWithCursor":{"sameCursor":true,"isComplete":true,"options":[[" ",2,[[" ","space","regex"]]],[",",2,[[",","comma","literal"]]],["😀",2,[["😀","word","regex"]]],["ünïcode",2,[["ünïcode","word","regex"]]],["日本",2,[["日本","word","regex"]]]],"error":null,"errorAtIndex":null,"ast":["日本",0,2],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["word",0,2],["anonymous-pattern-0",0,2],["words",0,2]]}},"tokensMessages":["Error at line 1, column 1. Hint: ünïcode, 日本","Error at line 1, column 1. Hint: ,,ünïcode,日本","Error at line 1, column 2. Hint: ,,ünïcode,日本","Error at line 1, column 3. Hint: ünïcode,日本","Error at line 1, column 3. Hint: ,,ünïcode,日本","Error at line 1, column 3. Hint: ünïcode,日本"],"expression":[{"text":"","isComplete":false,"options":[["-",0,[["-","-","literal"]]],["(",0,[["(","(","literal"]]]],"error":[0,0],"errorAtIndex":0,"ast":null,"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[]}},{"text":"1","isComplete":true,"options":[["*",1,[["*","*","literal"]]],["+",1,[["+","+","literal"]]]],"error":null,"errorAtIndex":null,"ast":["1",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[["-",0,0],["(",0,0]],"matched":[["num",0,1]]}},{"text":"1 ","isComplete":false,"options":[["+",2,[["+","+","literal"]]],["*",2,[["*","*","literal"]]]],"error":[2,2],"errorAtIndex":2,"ast":["1",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[["-",0,0],["(",0,0],["mul-delimiter",1,2],["add-delimiter",1,2]],"matched":[["num",0,1],["space",1,2],["space",1,2]]}},{"text":"1 +","isComplete":false,"options":[["-",3,[["-","-","literal"]]],["(",3,[["(","(","literal"]]]],"error":[3,3],"errorAtIndex":3,"ast":["1",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[["-",0,0],["(",0,0],["*",2,2]],"matched":[["num",0,1],["space",1,2],["space",1,2],["+",2,3],["add-delimiter",1,3]]}},{"text":"1 + (","isComplete":false,"options":[["-",5,[["-","-","literal"]]],["(",5,[["(","(","literal"]]]],"error":[5,5],"errorAtIndex":5,"ast":["1",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[["-",0,0],["(",0,0],["*",2,2],["-",4,4],["group",4,5],["num",4,4]],"matched":[["num",0,1],["space",1,2],["space",1,2],["+",2,3],["space",3,4],["add-delimiter",1,4],["(",4,5]]}},{"text":"(2","isComplete":false,"options":[[")",2,[[")",")","literal"]]],["*",2,[["*","*","literal"]]],["+",2,[["+","+","literal"]]]],"error":[2,2],"errorAtIndex":2,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["-",0,0],["-",1,1],["(",1,1],["group",0,2],["num",0,0],["expr",0,0]],"matched":[["(",0,1],["num",1,2]]}},{"text":"-","isComplete":false,"options":[["-",1,[["-","-","literal"]]],["(",1,[["(","(","literal"]]]],"error":[1,1],"errorAtIndex":1,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["expr",0,0]],"matched":[["-",0,1],["neg-prefix",0,1]]}}],"expressionStatic":{"isComplete":true,"options":[["*",1,[["*","*","literal"]]],["+",1,[["+","+","literal"]]]],"error":null,"errorAtIndex":null,"ast":["1",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[["-",0,0],["(",0,0]],"matched":[["num",0,1]]}},"expressionWithCursor":{"sameCursor":true,"isComplete":false,"options":[["+",2,[["+","+","literal"]]],["*",2,[["*","*","literal"]]]],"error":[2,2],"errorAtIndex":2,"ast":["1",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[["-",0,0],["(",0,0],["mul-delimiter",1,2],["add-delimiter",1,2]],"matched":[["num",0,1],["space",1,2],["space",1,2]]}},"expressionMessages":["Error at line 1, column 1. Hint: -, (","Error at line 1, column 1. Hint: *,+","Error at line 1, column 2. Hint: *","Error at line 1, column 3. Hint: -,(","Error at line 1, column 5. Hint: -,(","Error at line 1, column 2. Hint: *,+,)","Error at line 1, column 1. Hint: -,("],"mutual":[{"text":"","isComplete":false,"options":[["c",0,[["c","c","literal"]]],["d",0,[["d","d","literal"]]]],"error":[0,0],"errorAtIndex":0,"ast":null,"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[]}},{"text":"c","isComplete":true,"options":[],"error":null,"errorAtIndex":null,"ast":["c",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[["d",0,0],["b",0,0]],"matched":[["c",0,1]]}},{"text":"x","isComplete":false,"options":[["c",0,[["c","c","literal"]]],["d",0,[["d","d","literal"]]]],"error":[0,0],"errorAtIndex":0,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["d",0,0],["b",0,0],["c",0,0],["a",0,0]],"matched":[]}}],"mutualStatic":{"isComplete":true,"options":[],"error":null,"errorAtIndex":null,"ast":["c",0,1],"cursor":{"isRecording":true,"hasError":false,"errors":[["d",0,0],["b",0,0]],"matched":[["c",0,1]]}},"mutualWithCursor":{"sameCursor":true,"isComplete":false,"options":[["c",0,[["c","c","literal"]]],["d",0,[["d","d","literal"]]]],"error":[0,0],"errorAtIndex":0,"ast":null,"cursor":{"isRecording":true,"hasError":true,"errors":[["d",0,0],["b",0,0],["c",0,0],["a",0,0]],"matched":[]}},"mutualMessages":["Error at line 1, column 1. Hint: d, c, d, c","Error at line 1, column 1. Hint: ","Error at line 1, column 1. Hint: d, c, d, c"],"list":[{"text":"Ünal","isComplete":true,"options":[[", Ödön",4,[[", ","divider","literal"],["Ödön","Ödön","literal"]]],[", Ünal",4,[[", ","divider","literal"],["Ünal","Ünal","literal"]]],[", Zoë",4,[[", ","divider","literal"],["Zoë","Zoë","literal"]]]],"error":null,"errorAtIndex":null,"ast":["Ünal",0,4],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["Ünal",0,4],["names",0,4]]}},{"text":"Ünal, ","isComplete":false,"options":[["Ödön",6,[["Ödön","Ödön","literal"]]],["Ünal",6,[["Ünal","Ünal","literal"]]],["Zoë",6,[["Zoë","Zoë","literal"]]]],"error":[6,6],"errorAtIndex":6,"ast":["Ünal",0,4],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["Ünal",0,4],["divider",4,6],["names",0,4]]}},{"text":"Ünal, Zo","isComplete":false,"options":[["ë",8,[["Zoë","Zoë","literal"]]]],"error":[8,8],"errorAtIndex":8,"ast":["Ünal",0,4],"cursor":{"isRecording":true,"hasError":false,"errors":[["Ünal",6,6],["Ödön",6,6],["Zoë",6,7],["name",6,6]],"matched":[["Ünal",0,4],["divider",4,6],["names",0,4]]}},{"text":"Zoë, x","isComplete":false,"options":[["Ödön",5,[["Ödön","Ödön","literal"]]],["Ünal",5,[["Ünal","Ünal","literal"]]],["Zoë",5,[["Zoë","Zoë","literal"]]]],"error":[5,6],"errorAtIndex":5,"ast":["Zoë",0,3],"cursor":{"isRecording":true,"hasError":false,"errors":[["Ünal",0,0],["Ödön",0,0],["Ünal",5,5],["Ödön",5,5],["Zoë",5,5],["name",5,5]],"matched":[["Zoë",0,3],["divider",3,5],["names",0,3]]}}],"listStatic":{"isComplete":false,"options":[["Ödön",6,[["Ödön","Ödön","literal"]]],["Ünal",6,[["Ünal","Ünal","literal"]]],["Zoë",6,[["Zoë","Zoë","literal"]]]],"error":[6,6],"errorAtIndex":6,"ast":["Ünal",0,4],"cursor":{"isRecording":true,"hasError":false,"errors":[],"matched":[["Ünal",0,4],["divider",4,6],["names",0,4]]}},"listWithCursor":{"sameCursor":true,"isComplete":false,"options":[["ë",8,[["Zoë","Zoë","literal"]]]],"error":[8,8],"errorAtIndex":8,"ast":["Ünal",0,4],"cursor":{"isRecording":true,"hasError":false,"errors":[["Ünal",6,6],["Ödön",6,6],["Zoë",6,7],["name",6,6]],"matched":[["Ünal",0,4],["divider",4,6],["names",0,4]]}},"listMessages":["Error at line 1, column 4. Hint: ,","Error at line 1, column 6. Hint: Ünal,Ödön,Zoë","Error at line 1, column 7. Hint: Ünal,Ödön,Zoë","Error at line 1, column 6. Hint: Ünal,Ödön,Zoë"]};

/** What cpat answers where it deliberately differs from V12 (DESIGN §17):
 * - options are sorted as JS localeCompare sorts the characters cpat has
 *   collation tables for; anything else (an emoji) sorts after every
 *   letter, where ICU puts it first;
 * - a literal that runs out of text inside an emoji (`👍🏽` against a text
 *   ending in `👍`) fails at the emoji's start, where v12's UTF-16 compare
 *   fails at its low surrogate, so the error, `errorAtIndex` and that
 *   recorded error sit one unit earlier. */
function cpatExpects(v12) {
  const expected = structuredClone(v12);
  const isEmoji = text => /^\p{Extended_Pictographic}/u.test(text);
  const reorder = s => {
    if (s.options != null) {
      s.options = [...s.options.filter(o => !isEmoji(o[0])), ...s.options.filter(o => isEmoji(o[0]))];
    }
  };
  for (const value of Object.values(expected)) {
    (Array.isArray(value) ? value : [value]).forEach(reorder);
  }
  const cut = expected.greeting.find(s => s.text === "Jane loves 👍");
  cut.error = [11, 11];
  cut.errorAtIndex = 11;
  cut.cursor.errors = cut.cursor.errors.map(e => (e[0] === "👍🏽" && e[1] === 11 ? ["👍🏽", 11, 11] : e));
  return expected;
}

describe("AutoComplete compared with v12", () => {
  const actual = acCases(api);
  const expected = cpatExpects(V12);

  for (const key of Object.keys(V12)) {
    test(key, () => {
      assert.deepEqual(actual[key], expected[key]);
    });
  }

  test("the deviations adjust only emoji order and the one emoji cut short", () => {
    const changed = Object.keys(V12).filter(key => JSON.stringify(expected[key]) !== JSON.stringify(V12[key]));
    assert.deepEqual(changed, ["greeting", "tokens", "tokensStatic", "tokensWithCursor"]);
  });
});

describe("AutoComplete's option order for Latin letters", () => {
  // v12's suggestFor("") order for "<c>x" over every letter in
  // U+00C0–U+017F plus A a … Z z, recorded from a v12 run under Node 24
  // (the same file the core's autocomplete.rs test reads).
  const recorded = readFileSync(
    new URL("../../cpat-core/tests/fixtures/latin_order_v12.txt", import.meta.url),
    "utf8",
  ).split("\n").filter(line => line !== "");

  test("matches v12 for every letter in U+00C0–U+017F", () => {
    const letters = [];
    for (let code = 0xc0; code <= 0x17f; code++) {
      const letter = String.fromCodePoint(code);
      if (/\p{Alphabetic}/u.test(letter)) letters.push(letter);
    }
    for (let code = 65; code <= 90; code++) {
      letters.push(String.fromCharCode(code), String.fromCharCode(code + 32));
    }
    assert.equal(letters.length, 242);

    const pattern = new Expression("letters", letters.map((c, i) => new Literal(`l${i}`, `${c}x`)));
    const options = new AutoComplete(pattern).suggestFor("").options.map(o => o.text);
    assert.deepEqual(options, recorded);
  });
});

describe("AutoComplete's option order: compatibility and canonical equivalence", () => {
  // Expected orders recorded from v12 runs.
  const order = words =>
    new AutoComplete(new Expression("words", words.map((w, i) => new Literal(`w${i}`, w))))
      .suggestFor("").options.map(o => o.text);

  test("ĳ is ij told apart only by case", () => {
    assert.deepEqual(order(["IJ", "ĳ"]), ["ĳ", "IJ"]);
    assert.deepEqual(order(["íj", "ĳ", "ìj", "Ĳ"]), ["ĳ", "Ĳ", "íj", "ìj"]);
  });

  test("marks weigh in canonical order, however typed", () => {
    assert.deepEqual(
      order(["â̧", "ã", "â̧"]),
      ["ã", "â̧", "â̧"],
    );
    assert.deepEqual(order(["ã", "Â̧"]), ["ã", "Â̧"]);
  });
});

describe("AutoComplete over a mark no leaf covers", () => {
  // A literal matches the base of a letter + combining mark; the next leaf
  // starts after the whole grapheme. v12's ast.value leaves the mark out, so
  // the text is not complete, as exec says. Expected values recorded from v12.
  const suggest = (grammar, rule, text) => {
    const pattern = api.Grammar.parseString(grammar)[rule];
    const s = new AutoComplete(pattern).suggestFor(text);
    return {
      exec: pattern.exec(text).ast,
      complete: s.isComplete,
      at: s.errorAtIndex,
      error: s.error && [s.error.startIndex, s.error.lastIndex],
      options: s.options.map(o => [o.text, o.startIndex]),
    };
  };

  test("between two leaves of a sequence", () => {
    assert.deepEqual(suggest('a = "a"\nseq = a + "b"', "seq", "áb"), {
      exec: null, complete: false, at: 3, error: [3, 3], options: [],
    });
  });

  test("inside a repeat", () => {
    assert.deepEqual(suggest('a = "a"\nrep = (a)+', "rep", "aáa"), {
      exec: null, complete: false, at: 4, error: [4, 4], options: [["a", 4]],
    });
  });
});
