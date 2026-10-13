// clarity-pattern-parser v12's src/grammar/Grammar.test.ts, run against js/index.mjs with its
// assertions as written (jest's `expect`, from ./testing/expect.mjs). Run
// with `node --test js/*.test.mjs` after building pkg-node. Where cpat
// deliberately differs, the assertion says so (DESIGN §22).
import { describe, test, expect } from "./testing/expect.mjs";
import { Sequence, Literal, Not, Reference, Regex, Repeat, Grammar, Optional, Context, createPatternsTemplate, patterns, Expression, Cursor } from "./index.mjs";


describe("Grammar", () => {
  test("Literal", () => {
    const expression = `
            name = "John"
        `;

    const patterns = Grammar.parseString(expression);
    const namePattern = patterns["name"];
    const name = new Literal("name", "John");
    const expected = new Context("name", name);

    expect(namePattern.isEqual(expected)).toBeTruthy();
  });

  test("Literal With Escaped Characters", () => {
    const expression = `
            chars = "\\n\\r\\t\\b\\f\\v\\0\\x00\\u0000\\"\\\\"
        `;
    const patterns = Grammar.parseString(expression);
    const namePattern = patterns["chars"];
    const chars = new Literal("chars", '\n\r\t\b\f\v\0\x00\u0000"\\');
    const expected = new Context("chars", chars);

    expect(namePattern.isEqual(expected)).toBeTruthy();
  });

  test("Literal With Escaped Quotes", () => {
    const expression = `
            content = "With Con\\"tent"
        `;
    const patterns = Grammar.parseString(expression);
    const namePattern = patterns["content"];
    const content = new Literal("content", 'With Con"tent');
    const expected = new Context(`content`, content);

    expect(namePattern.isEqual(expected)).toBeTruthy();
  });

  test("Regex", () => {
    const expression = `
            name = /\\w/
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["name"];
    const name = new Regex("name", "\\w");
    const expected = new Context(`name`, name);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  describe("literal escape decoding", () => {
    // String.raw keeps the cpat source exactly as written in a .cpat file.
    const decode = (cpatLiteral        ) =>
      (Grammar.parseString(`x = ${cpatLiteral}`).x.children[0]           ).token;

    test("an escaped backslash followed by n is a backslash and an n", () => {
      expect(decode(String.raw`"\\n"`)).toBe("\\n");
    });

    test("two escaped backslashes are two backslashes", () => {
      expect(decode(String.raw`"\\\\"`)).toBe("\\\\");
    });

    test("an escaped backslash then an escaped quote", () => {
      expect(decode(String.raw`"\\\""`)).toBe('\\"');
    });

    test("each documented escape", () => {
      expect(decode(String.raw`"\n\r\t\b\f\v\0"`)).toBe("\n\r\t\b\f\v\0");
      expect(decode(String.raw`"\x41\x7a"`)).toBe("Az");
      expect(decode(String.raw`"\u00e9\u2603"`)).toBe("\u00e9\u2603");
      expect(decode(String.raw`"\"quoted\""`)).toBe('"quoted"');
    });

    test("mixed sequences decode left to right", () => {
      expect(decode(String.raw`"a\\tb\tc\\\nd"`)).toBe("a\\tb\tc\\\nd");
      expect(decode(String.raw`"\\x41\x41"`)).toBe("\\x41A");
      expect(decode(String.raw`"\\u0041\u0041"`)).toBe("\\u0041A");
    });

    test("any other escaped character means itself", () => {
      expect(decode(String.raw`"\q\/\-"`)).toBe("q/-");
    });

    test("an incomplete \\x or \\u escape means the letter itself", () => {
      // (plain strings: TypeScript rejects these escapes even in String.raw)
      expect(decode('"\\xZZ"')).toBe("xZZ");
      expect(decode('"\\u12"')).toBe("u12");
    });
  });

  test("Or", () => {
    const expression = `
            john = "John"
            jane = "Jane"
            names = john | jane
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["names"];
    const john = new Literal("john", "John");
    const jane = new Literal("jane", "Jane");
    const names = new Expression("names", [john, jane]);
    const expected = new Context("names", names, [john, jane]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("And", () => {
    const expression = `
            space = " "
            first-name = /\\w/
            last-name = /\\w/
            full-name = first-name + space + last-name
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["full-name"];
    const space = new Literal("space", " ");
    const firstName = new Regex("first-name", "\\w");
    const lastName = new Regex("last-name", "\\w");
    const fullName = new Sequence("full-name", [firstName, space, lastName]);
    const expected = new Context("full-name", fullName, [space, firstName, lastName]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("And With Optional Pattern", () => {
    const expression = `
            space = " "
            first-name = /\\w/
            last-name = /\\w/
            middle-name = /\\w/
            middle-name-with-space = middle-name + space
            full-name = first-name + space + middle-name-with-space? + last-name
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["full-name"];
    const space = new Literal("space", " ");
    const firstName = new Regex("first-name", "\\w");
    const lastName = new Regex("last-name", "\\w");
    const middleName = new Regex("middle-name", "\\w");
    const middleNameWithSpace = new Sequence("middle-name-with-space", [
      middleName,
      space,
    ]);
    const optionalMiddleNameWithSpace = new Optional(
      "optional-middle-name-with-space",
      middleNameWithSpace
    );
    const fullName = new Sequence("full-name", [
      firstName,
      space,
      optionalMiddleNameWithSpace,
      lastName,
    ]);
    const expected = new Context("full-name", fullName, [
      space,
      firstName,
      lastName,
      middleName,
      middleNameWithSpace,
    ]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("And With Not Pattern", () => {
    const expression = `
            space = " "
            first-name = /\\w/
            last-name = /\\w/
            middle-name = /\\w/
            jack = "Jack"
            middle-name-with-space = middle-name + space
            full-name = !jack + first-name + space + middle-name-with-space? + last-name
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["full-name"];
    const space = new Literal("space", " ");
    const firstName = new Regex("first-name", "\\w");
    const lastName = new Regex("last-name", "\\w");
    const middleName = new Regex("middle-name", "\\w");
    const jack = new Literal("jack", "Jack");
    const middleNameWithSpace = new Sequence("middle-name-with-space", [
      middleName,
      space,
    ]);
    const optionalMiddleNameWithSpace = new Optional(
      "optional-middle-name-with-space",
      middleNameWithSpace
    );
    const fullName = new Sequence("full-name", [
      new Not("not-jack", jack),
      firstName,
      space,
      optionalMiddleNameWithSpace,
      lastName,
    ]);
    const expected = new Context("full-name", fullName, [
      space,
      firstName,
      lastName,
      middleName,
      jack,
      middleNameWithSpace,
    ]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat", () => {
    const expression = `
            digit = /\\d/
            digits = (digit)+
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d");
    const digits = new Repeat("digits", digit);
    const expected = new Context("digits", digits, [digit]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat Zero Or More", () => {
    const expression = `
            digit = /\\d/
            digits = (digit)*
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d");
    const digits = new Optional(
      "optional-digits",
      new Repeat("digits", digit, { min: 0 })
    );

    const expected = new Context("optional-digits", digits, [digit]);
    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat Lower Limit", () => {
    const expression = `
            digit = /\\d+/
            digits = (digit){1,}
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d+");
    const digits = new Repeat("digits", digit, { min: 1 });
    const expected = new Context("digits", digits, [digit]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat Bounded", () => {
    const expression = `
            digit = /\\d+/
            digits = (digit){1,3}
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d+");
    const digits = new Repeat("digits", digit, { min: 1, max: 3 });
    const expected = new Context("digits", digits, [digit]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat Upper Limit", () => {
    const expression = `
            digit = /\\d+/
            digits = (digit){,3}
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d+");
    const digits = new Repeat("digits", digit, { min: 0, max: 3 });
    const expected = new Context("digits", digits, [digit]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat Exact", () => {
    const expression = `
            digit = /\\d+/
            digits = (digit){3}
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d+");
    const digits = new Repeat("digits", digit, { min: 3, max: 3 });
    const expected = new Context("digits", digits, [digit]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat Divider", () => {
    const expression = `
            digit = /\\d+/
            comma = ","
            digits = (digit, comma){3}
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d+");
    const comma = new Literal("comma", ",");
    const digits = new Repeat("digits", digit, { divider: comma, min: 3, max: 3 });
    const expected = new Context("digits", digits, [digit, comma]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat Divider With Trim Divider", () => {
    const expression = `
            digit = /\\d+/
            comma = ","
            digits = (digit, comma trim)+
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d+");
    const comma = new Literal("comma", ",");
    const digits = new Repeat("digits", digit, {
      divider: comma,
      min: 1,
      trimDivider: true,
    });
    const expected = new Context("digits", digits, [digit, comma]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Repeat Divider With Trim Divider And Bounds", () => {
    const expression = `
            digit = /\\d+/
            comma = ","
            digits = (digit, comma trim){3, 3}
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["digits"];
    const digit = new Regex("digit", "\\d+");
    const divider = new Literal("comma", ",");
    const digits = new Repeat("digits", digit, {
      divider,
      min: 3,
      max: 3,
      trimDivider: true,
    });
    const expected = new Context("digits", digits, [digit, divider]);

    expect(pattern.isEqual(expected)).toBeTruthy();
  });

  test("Reference", () => {
    const expression = `
            digit = /\\d+/
            divider = /\\s*,\\s*/
            open-bracket = "["
            close-bracket = "]"
            spaces = /\\s+/
            items = digit | array
            array-items = (items, divider trim)*
            array = open-bracket + spaces? + array-items + spaces? + close-bracket
        `;

    const patterns = Grammar.parseString(expression);
    const pattern = patterns["array"]           ;

    const text = "[1, []]";
    const result = pattern.exec(text, true);

    expect(result.ast?.value).toEqual("[1, []]");
  });

  test("Alias", () => {
    const expression = `
            name = /regex/
            alias = name
        `;

    const patterns = Grammar.parseString(expression);

    const name = patterns["name"];
    const expectedName = new Regex("name", "regex");

    const alias = patterns["alias"];

    const expectedAlias = new Regex("alias", "regex");

    const contextualAlias = new Context("alias", expectedAlias, [expectedName]);
    const contextualName = new Context("name", expectedName, [expectedAlias]);

    expect(name.isEqual(contextualName)).toBeTruthy();
    expect(alias.isEqual(contextualAlias)).toBeTruthy();
  });

  test("Bad Grammar At Beginning", () => {
    expect(() => {
      const expression = `//`;
      Grammar.parseString(expression);
    }).toThrow();
  });

  test("Bad Grammar Further In", () => {
    expect(() => {
      const expression = `name = /\\w/
                age = /()
            `;
      Grammar.parseString(expression);
    }).toThrow();
  });

  test("Import", async () => {
    const importExpression = `first-name = "John"`;
    const expression = `
        import { first-name } from "some/path/to/file.cpat"
        last-name = "Doe"
        space = " "
        full-name = first-name + space + last-name
        `;

    function resolveImport(resource        ) {
      expect(resource).toBe("some/path/to/file.cpat");
      return Promise.resolve({ expression: importExpression, resource });
    }

    const patterns = await Grammar.parse(expression, { resolveImport });
    const fullname = patterns["full-name"]           ;
    const result = fullname.exec("John Doe");

    expect(result?.ast?.value).toBe("John Doe");
  });

  test("Imports", async () => {
    const importExpression = `first-name = "John"`;
    const spaceExpression = `space = " "`;

    const pathMap                         = {
      "space.cpat": spaceExpression,
      "first-name.cpat": importExpression,
    };

    const expression = `
        import { first-name } from "first-name.cpat"
        import { space } from "space.cpat"
        last-name = "Doe"
        full-name = first-name + space + last-name
        `;
    function resolveImport(resource        ) {
      return Promise.resolve({ expression: pathMap[resource], resource });
    }

    const patterns = await Grammar.parse(expression, { resolveImport });
    const fullname = patterns["full-name"]           ;
    const result = fullname.exec("John Doe");
    expect(result?.ast?.value).toBe("John Doe");
  });

  test("Imports with Params", async () => {
    const importExpression = `first-name = "John"`;
    const spaceExpression = `
        use params { custom-space }
        space = custom-space
        `;
    const expression = `
        import { first-name } from "first-name.cpat"
        import { space } from "space.cpat" with params {
          custom-space = "  "
        }
        last-name = "Doe"
        full-name = first-name + space + last-name
        `;

    const pathMap                         = {
      "space.cpat": spaceExpression,
      "first-name.cpat": importExpression,
    };

    function resolveImport(resource        ) {
      return Promise.resolve({ expression: pathMap[resource], resource });
    }

    const patterns = await Grammar.parse(expression, { resolveImport });
    const fullname = patterns["full-name"]           ;
    const result = fullname.exec("John  Doe");
    expect(result?.ast?.value).toBe("John  Doe");
  });

  test("Export Name", async () => {
    const expression = `
        import { use-this } from "resource1"
        import {name} from "resource2" with params {
           use-this   
        }

        name
        `;

    const resource1 = `
        use-this = "Use This"
        `;

    const resource2 = `
        use params {
            use-this
        }
        name = use-this
        `;

    const pathMap                         = {
      resource1: resource1,
      resource2: resource2,
    };

    function resolveImport(resource        ) {
      return Promise.resolve({ expression: pathMap[resource], resource });
    }
    const patterns = await Grammar.parse(expression, { resolveImport });
    const pattern = patterns["name"]           ;

    const result = pattern.exec("Use This");

    expect(result.ast?.value).toBe("Use This");
  });

  test("Import Alias", async () => {
    const expression = `
        import { value as alias } from "resource1"
        import { export-value } from "resource2" with params {
            param = alias
        }
        name = export-value
        `;

    const resource1 = `
        value = "Value"
        `;

    const resource2 = `
        use params {
            param
        }
        export-value = param
        `;

    const pathMap                         = {
      resource1: resource1,
      resource2: resource2,
    };

    function resolveImport(resource        ) {
      return Promise.resolve({ expression: pathMap[resource], resource });
    }
    const patterns = await Grammar.parse(expression, { resolveImport });
    const pattern = patterns["name"]           ;
    const result = pattern.exec("Value");
    expect(result.ast?.value).toBe("Value");
  });

  test("Default Params Resolves to Default Value", async () => {
    const expression = `
        use params {
            value = default-value
        }

        default-value = "DefaultValue"
        alias = value
        `;

    function resolveImport(_        ) {
      return Promise.reject(new Error("No Import"));
    }

    const patterns = await Grammar.parse(expression, { resolveImport });
    const pattern = patterns["alias"]           ;

    const result = pattern.exec("DefaultValue");
    expect(result.ast?.value).toBe("DefaultValue");
  });

  test("Default Params Resolves to params imported", async () => {
    const expression = `
        use params {
            value = default-value
        }

        default-value = "DefaultValue"
        alias = value
        `;

    function resolveImport(_        ) {
      return Promise.reject(new Error("No Import"));
    }

    const patterns = await Grammar.parse(expression, {
      resolveImport,
      params: [new Literal("value", "Value")],
    });
    const pattern = patterns["alias"]           ;

    const result = pattern.exec("Value");
    expect(result.ast?.value).toBe("Value");
  });

  test("Default Params Resolves to imported default value", async () => {
    const expression = `
        import { my-value as default-value } from "resource1"
        use params {
            value = default-value
        }

        default-value = "DefaultValue"
        alias = value
        `;

    const resource1 = `
            my-value = "MyValue"
        `;

    const pathMap                         = {
      resource1: resource1,
    };

    function resolveImport(resource        ) {
      return Promise.resolve({ expression: pathMap[resource], resource });
    }

    const patterns = await Grammar.parse(expression, { resolveImport });
    const pattern = patterns["alias"]           ;

    const result = pattern.exec("MyValue");
    expect(result.ast?.value).toBe("MyValue");
  });

  test("Anonymous Patterns", () => {
    const expression = `
            complex-expression = !"NOT_THIS" + "Text"? + /regex/ + ("Text" | /regex/ | (pattern)+) + (pattern | pattern)
            pattern = "p"
        `;

    const patterns = Grammar.parseString(expression);
    const expected = new Context(
      "complex-expression",
      new Sequence("complex-expression", [
        new Not("not-NOT_THIS", new Literal("NOT_THIS", "NOT_THIS")),
        new Optional("Text", new Literal("Text", "Text")),
        new Regex("regex", "regex"),
        new Expression("anonymous", [
          new Literal("Text", "Text"),
          new Regex("regex", "regex"),
          new Repeat("anonymous", new Reference("pattern")),
        ]),
        new Expression("anonymous", [new Reference("pattern"), new Reference("pattern")]),
      ])
    );

    expect(patterns["complex-expression"].isEqual(expected)).toBeTruthy();
  });

  test("Grammar With Spaces", () => {
    const expression = `
        john = "John"
            
        jane = "Jane"
        `;
    const patterns = Grammar.parseString(expression);
    expect(patterns.john).not.toBeNull();
    expect(patterns.jane).not.toBeNull();
  });

  test("Grammar Import", async () => {
    const importExpression = `first-name = "John"`;
    const spaceExpression = `
        use params { custom-space }
        space = custom-space
        `;
    const expression = `
        use params {
          custom-space
        }
        import { first-name } from "first-name.cpat"
        import { space } from "space.cpat" with params {
          custom-space = custom-space
        }
        last-name = "Doe"
        full-name = first-name + space + last-name
        `;

    const pathMap                         = {
      "space.cpat": spaceExpression,
      "first-name.cpat": importExpression,
      "root.cpat": expression,
    };

    function resolveImport(resource        ) {
      return Promise.resolve({ expression: pathMap[resource], resource });
    }

    const patterns = await Grammar.import("root.cpat", {
      resolveImport,
      params: [new Literal("custom-space", "  ")],
    });
    const fullname = patterns["full-name"]           ;
    const result = fullname.exec("John  Doe");
    expect(result?.ast?.value).toBe("John  Doe");
  });

  test("Expression Pattern", () => {
    const { expression } = patterns`
            variables = "a" | "b" | "c"
            ternary = expression + " ? " + expression + " : " + expression
            expression = ternary | variables

            bad-ternary = bad-expression + " ? " + bad-expression + " : " + bad-expression
            bad-expression = bad-ternary | bad-ternary
        `;
    const result = expression.exec("a ? b : c");

    expect(result.cursor.hasError).toBe(false);
    expect(result.ast?.value).toBe("a ? b : c");
    expect(result.ast?.name).toBe("ternary");
    expect(result.ast?.children.map(c => c.value)).toEqual(["a", " ? ", "b", " : ", "c"]);
  });

  test("Expression Pattern With Right Association", () => {
    const { expression } = patterns`
            variables = "a" | "b" | "c" | "d" | "e"
            ternary = expression + " ? " + expression + " : " + expression
            expression = ternary right | variables
        `;
    const result = expression.exec("a ? b : c ? d : e");

    expect(result.cursor.hasError).toBe(false);
    expect(result.ast?.value).toBe("a ? b : c ? d : e");

    // Right associative: the nested ternary is the LAST operand, so this reads
    // as "a ? b : (c ? d : e)". Left association would instead have nested the
    // inner ternary into the FIRST operand.
    expect(result.ast?.name).toBe("ternary");
    expect(result.ast?.children[0].value).toBe("a");

    const nested = result.ast?.children[4];
    expect(nested?.name).toBe("ternary");
    expect(nested?.value).toBe("c ? d : e");
  });

  test("Decorators", () => {
    const { spaces } = patterns`
            @tokens([" "])
            spaces = /\\s+/
        `;

    expect(spaces.getTokens()).toEqual([" "]);
  });

  test("Decorators On Take Until", () => {
    const { scriptText } = patterns`
            @tokens(["</script"])
            script-text = ?->| "</script"
        `;

    // The decorator finds its target structurally, so any pattern that accepts
    // tokens can use it — not just regex.
    expect(scriptText.getTokens()).toEqual(["</script"]);
  });

  test("@tokens on a pattern that can't take tokens is a build error", () => {
    // A Literal derives its own token; a Sequence offers its children's.
    expect(
      () => patterns`
            @tokens(["ignored"])
            name = "John"
        `
    ).toThrow(
      "[Invalid Grammar] `@tokens` can't apply to `name` (a literal) — only patterns with setTokens accept tokens"
    );
    expect(
      () => patterns`
            @tokens(["x"])
            pair = "a" + "b"
        `
    ).toThrow("`@tokens` can't apply to `pair` (a sequence)");
  });

  test("@tokens with no argument is a build error", () => {
    expect(
      () => patterns`
            @tokens()
            spaces = /\\s+/
        `
    ).toThrow("[Invalid Grammar] `@tokens` on `spaces` expects an array of strings");
  });

  test("@tokens with a non-array argument is a build error", () => {
    expect(
      () => patterns`
            @tokens("Bad")
            spaces = /\\s+/
        `
    ).toThrow("`@tokens` on `spaces` expects an array of strings");
  });

  test("@tokens with a non-string token is a build error", () => {
    expect(
      () => patterns`
            @tokens([" ", 2])
            spaces = /\\s+/
        `
    ).toThrow("`@tokens` on `spaces` expects an array of strings");
  });

  test("an unknown decorator is a build error", () => {
    expect(
      () => patterns`
            @bad-decorator()
            spaces = /\\s+/
        `
    ).toThrow("[Invalid Grammar] unknown decorator `@bad-decorator` on `spaces`");
    expect(
      () => patterns`
            @tokn
            keyword = /[a-z]+/
        `
    ).toThrow("unknown decorator `@tokn` on `keyword`");
  });

  test("Decorators On Multiple Patterns", () => {
    const { spaces, digits } = patterns`
            @tokens([" "])
            spaces = /\\s+/

            @tokens(["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"])
            digits = /\\d+/
        `;

    expect(spaces.getTokens()).toEqual([" "]);
    expect(digits.getTokens()).toEqual([
      "0",
      "1",
      "2",
      "3",
      "4",
      "5",
      "6",
      "7",
      "8",
      "9",
    ]);
  });

  test("Decorators On Multiple Patterns And Comments", () => {
    const { spaces, digits } = patterns`
            #Comment
            @tokens([" "])
            spaces = /\\s+/

            #Comment
            @tokens(["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"])
            #Comment
            digits = /\\d+/
        `;

    expect(spaces.getTokens()).toEqual([" "]);
    expect(digits.getTokens()).toEqual([
      "0",
      "1",
      "2",
      "3",
      "4",
      "5",
      "6",
      "7",
      "8",
      "9",
    ]);
  });

  test("Custom Named Decorator", () => {
    const allRecordedPatterns           = [];
    const patterns = createPatternsTemplate({
      decorators: {
        record: pattern => {
          allRecordedPatterns.push(pattern.name);
        },
      },
    });

    patterns`
            @record
            spaces = /\\s+/

            @record
            digits = /\\d+/
        `;

    expect(allRecordedPatterns).toEqual(["spaces", "digits"]);
  });

  test("Decorator With Empty Object Literal", () => {
    const received              = [];
    const withMethod = createPatternsTemplate({
      decorators: { method: (_p, arg) => received.push(arg             ) },
    });
    const { spaces } = withMethod`
            @method({})
            spaces = /\\s+/
        `;

    expect(received).toEqual([{}]);
    expect(spaces).toBeDefined();
    expect(spaces.name).toBe("spaces");
    expect(spaces.exec("   ").ast?.value).toBe("   ");
  });

  test("Decorator With Object Literal", () => {
    const received              = [];
    const withMethod = createPatternsTemplate({
      decorators: { method: (_p, arg) => received.push(arg             ) },
    });
    const { spaces } = withMethod`
            @method({"prop": 2})
            spaces = /\\s+/
        `;

    expect(received).toEqual([{ prop: 2 }]);
    expect(spaces).toBeDefined();
    expect(spaces.name).toBe("spaces");
    expect(spaces.exec("  ").ast?.value).toBe("  ");
  });

  test("Take Until", () => {
    const { scriptText } = patterns`
            script-text = ?->| "</script"
        `;
    const result = scriptText.parse(new Cursor("function(){}</script"));

    expect(result?.value).toBe("function(){}");
  });

  test("Import Sync", () => {
    function resolveImportSync(path        , _importer               ) {
      return {
        expression: pathMap[path],
        resource: path,
      };
    }

    const rootExpression = `
        import { name } from "first-name.cpat"
        full-name = name
        `;

    const firstNameExpression = `
        import { last-name } from "last-name.cpat"
        first-name = "John"
        name = first-name + " " +  last-name
        `;

    const lastNameExpression = `
        last-name = "Doe"
        `;

    const pathMap                         = {
      "first-name.cpat": firstNameExpression,
      "last-name.cpat": lastNameExpression,
    };

    const patterns = Grammar.parseString(rootExpression, {
      resolveImportSync: resolveImportSync,
      originResource: "/root.cpat",
    });

    expect(patterns["full-name"]).not.toBeNull();
    const result = patterns["full-name"].exec("John Doe");
    expect(result?.ast?.value).toBe("John Doe");
  });

  // --- Unknown pattern names ---

  describe("unknown pattern names are build errors", () => {
    test("a reference to a name defined nowhere names the name and the rule", () => {
      expect(() => Grammar.parseString(`x = missing + "b"`)).toThrow(
        "[Invalid Grammar] unknown pattern `missing` used in `x`"
      );
    });

    test("nested references are checked too", () => {
      expect(() =>
        Grammar.parseString(`
            a = "a"
            x = a + ("b" | (nope)+)
        `)
      ).toThrow("unknown pattern `nope` used in `x`");
    });

    test("a forward reference to a pattern defined later is fine", () => {
      const p = Grammar.parseString(`
            x = later + "!"
            later = "a"
        `);
      expect(p.x.exec("a!").ast?.value).toBe("a!");
    });

    test("an alias of an unknown name is an error", () => {
      expect(() => Grammar.parseString(`x = missing`)).toThrow(
        "unknown pattern `missing` used in `x`"
      );
    });

    test("exporting an unknown name is an error", () => {
      expect(() => Grammar.parseString(`missing`)).toThrow(
        "unknown pattern `missing` used in `export missing`"
      );
    });

    test("a `use params` default that names nothing is an error", () => {
      expect(() =>
        Grammar.parseString(`
            use params { value = nowhere }
            x = value
        `)
      ).toThrow("unknown pattern `nowhere` used in `use params { value }`");
    });

    test("a declared param that is neither supplied nor defaulted is an error when used", () => {
      expect(() =>
        Grammar.parseString(`
            use params { value }
            x = value + "!"
        `)
      ).toThrow("unknown pattern `value` used in `x`");

      const p = Grammar.parseString(
        `
            use params { value }
            x = value + "!"
        `,
        { params: [new Literal("value", "v")] }
      );
      expect(p.x.exec("v!").ast?.value).toBe("v!");
    });

    test("imported and param names count as defined", async () => {
      const p = await Grammar.parse(
        `
            import { a } from "lib"
            x = a + b
        `,
        {
          params: [new Literal("b", "B")],
          resolveImport: (resource        ) =>
            Promise.resolve({ expression: `a = "A"`, resource }),
        }
      );
      expect(p.x.exec("AB").ast?.value).toBe("AB");
    });

    test("a hand-built Reference stays lazy", () => {
      const ref = new Reference("not-there");
      expect(() => ref.exec("x")).toThrow(
        "Couldn't find 'not-there' pattern within tree."
      );
    });
  });

  // --- Shadowed literals ---

  describe("shadowed literals are build errors", () => {
    test("a literal that a shorter, earlier literal shadows", () => {
      expect(() => Grammar.parseString(`x = "=" | "=="`)).toThrow(
        '[Invalid Grammar] `"=="` can never match after `"="` in `x`; put the longer literal first.'
      );
    });

    test("the longer literal first is fine", () => {
      const p = Grammar.parseString(`x = "==" | "="`);
      expect(p.x.exec("==").ast?.value).toBe("==");
      expect(p.x.exec("=").ast?.value).toBe("=");
    });

    test("not adjacent, and inside a nested choice", () => {
      expect(() => Grammar.parseString(`x = "a" + ("<" | ">" | "<=")`)).toThrow(
        '`"<="` can never match after `"<"` in `x`'
      );
    });

    test("looks through rule names, including ones defined later", () => {
      expect(() =>
        Grammar.parseString(`
            lt = "<"
            op = lt | le
            le = "<="
        `)
      ).toThrow('`"<="` can never match after `"<"` in `op`');
    });

    test("looks through aliases", () => {
      expect(() =>
        Grammar.parseString(`
            less = "<"
            lt = less
            op = lt | "<="
        `)
      ).toThrow('`"<="` can never match after `"<"` in `op`');
    });

    test("looks through imported literals", () => {
      expect(() =>
        Grammar.parseString(
          `
            import { lt } from "ops"
            op = lt | "<="
        `,
          { resolveImportSync: resource => ({ expression: `lt = "<"`, resource }) }
        )
      ).toThrow('`"<="` can never match after `"<"` in `op`');
    });

    test("literals that are not prefixes of each other are fine", () => {
      const p = Grammar.parseString(`x = "ab" | "ac" | "b"`);
      expect(p.x.exec("ac").ast?.value).toBe("ac");
    });

    test("a regex or sequence before a longer literal is not checked", () => {
      const p = Grammar.parseString(`x = /</ | "<=" | "<" + "!"`);
      expect(p.x.exec("<").ast?.value).toBe("<");
    });
  });

  // --- Alias cycles ---

  describe("alias cycles are build errors", () => {
    test("two aliases naming each other", () => {
      expect(() =>
        Grammar.parseString(`
            a = b
            b = a
        `)
      ).toThrow("[Invalid Grammar] alias cycle `a` -> `b` -> `a`");
    });

    test("a rule that aliases itself", () => {
      expect(() => Grammar.parseString(`a = a`)).toThrow(
        "[Invalid Grammar] alias cycle `a` -> `a`"
      );
    });

    test("a longer chain, entered from a rule outside the cycle", () => {
      expect(() =>
        Grammar.parseString(`
            entry = a
            a = b
            b = c
            c = a
        `)
      ).toThrow("[Invalid Grammar] alias cycle `a` -> `b` -> `c` -> `a`");
    });

    test("a parenthesized alias counts", () => {
      expect(() =>
        Grammar.parseString(`
            a = (b)
            b = a
        `)
      ).toThrow("alias cycle `a` -> `b` -> `a`");
    });

    test("alias chains that end in a real pattern are fine", () => {
      const p = Grammar.parseString(`
            a = b
            b = c
            c = "x"
        `);
      expect(p.a.exec("x").ast?.value).toBe("x");
    });

    test("a forward alias keeps its decorators", () => {
      const p = Grammar.parseString(`
            @tokens(["xx"])
            a = b
            b = /x+/
        `);
      expect(p.a.exec("xxx").ast?.value).toBe("xxx");
      expect(p.a.getTokens()).toEqual(["xx"]);
    });

    test("a rule may pass a same-named param through", () => {
      const p = Grammar.parseString(
        `
            use params { space }
            space = space
        `,
        { params: [new Literal("space", " ")] }
      );
      expect(p.space.exec(" ").ast?.value).toBe(" ");
    });

    test("recursion through a real pattern is not an alias cycle", () => {
      const p = Grammar.parseString(`
            a = b
            b = "(" + a + ")" | "x"
        `);
      expect(p.a.exec("((x))").ast?.value).toBe("((x))");
    });
  });

  // --- Deterministic anonymous names ---

  describe("anonymous pattern names", () => {
    const source = `
            x = ("a" | "b") + ("c" + "d") + ("e" | "f")
        `;

    function nodeNames(node             )           {
      if (node == null) {
        return [];
      }
      return [node.name, ...node.children.flatMap(nodeNames)];
    }

    function patternNames(pattern         )           {
      return [pattern.name, ...pattern.children.flatMap(patternNames)];
    }

    function build(cpat        ) {
      const p = Grammar.parseString(cpat);
      const ast = p.x.exec("acde").ast;
      return { patterns: patternNames(p.x), nodes: nodeNames(ast) };
    }

    test("building the same grammar twice yields identical names", () => {
      const first = build(source);
      Grammar.parseString(`other = ("q" + "r") | ("s" + "t")`);
      const second = build(source);

      expect(first.nodes.length).toBeGreaterThan(1);
      expect(second.patterns).toEqual(first.patterns);
      expect(second.nodes).toEqual(first.nodes);
    });

    test("names are numbered from the start of each build", () => {
      Grammar.parseString(`other = ("q" + "r") | ("s" + "t")`);
      const { patterns } = build(source);

      // The numbering is per build: it does not carry on from earlier builds.
      expect(patterns.filter(n => n.startsWith("anonymous-pattern-"))).toEqual([
        "anonymous-pattern-1",
        "anonymous-pattern-5",
        "anonymous-pattern-9",
      ]);
    });

    test("an imported grammar's anonymous names do not collide with the importer's", () => {
      const pathMap                         = {
        "lib.cpat": `pair = ("q" | "r") + "s"`,
      };
      const p = Grammar.parseString(
        `
            import { pair } from "lib.cpat"
            x = (pair | "z") + ("y" | "w")
        `,
        {
          resolveImportSync: (resource        ) => ({
            expression: pathMap[resource],
            resource,
          }),
        }
      );

      expect(p.x.exec("qsy").ast?.value).toBe("qsy");
      expect(p.x.exec("zw").ast?.value).toBe("zw");
    });
  });

  // --- Every `|` is an Expression ---

  test("`|` builds the same pattern regardless of definition order", () => {
    const before = Grammar.parseString(`
            a = "a"
            b = "b"
            x = a | b
        `);
    const after = Grammar.parseString(`
            a = "a"
            x = a | b
            b = "b"
        `);

    expect(before.x.children[0].type).toBe("expression");
    expect(after.x.children[0].type).toBe(before.x.children[0].type);

    for (const text of ["a", "b", "c", "ab", ""]) {
      const left = before.x.exec(text);
      const right = after.x.exec(text);

      if (left.ast == null) {
        expect(right.ast).toBeNull();
      } else {
        expect(right.ast?.isEqual(left.ast)).toBe(true);
      }
      expect(right.cursor.furthestError?.startIndex).toBe(
        left.cursor.furthestError?.startIndex
      );
    }
    expect(after.x.exec("b").ast?.name).toBe("b");
    expect(after.x.exec("b").ast?.type).toBe("literal");
  });

  test("an atom-only `|` returns the matched alternative's node unwrapped", () => {
    const { x } = patterns`
            x = "a" | "b"
        `;
    const { ast } = x.exec("b");

    expect(ast?.type).toBe("literal");
    expect(ast?.name).toBe("b");
  });

  test("mutually recursive choices build and parse", () => {
    const p = Grammar.parseString(`
            a = b | "c"
            b = a | "d"
        `);

    expect(p.a.exec("c").ast?.value).toBe("c");
    expect(p.a.exec("d").ast?.value).toBe("d");
    expect(p.a.exec("e").ast).toBeNull();
  });

  // --- Greedy options (removed in v12) ---

  const greedyRemovedMessage =
    "greedy options (`<|>`) were removed in v12; order longer alternatives first, or use a regex such as /==|=/";

  test("greedy options `<|>` are rejected with a message that teaches the fix", () => {
    expect(() =>
      Grammar.parseString(`
            hi = "hi"
            hiya = "hiya"
            greeting = hi <|> hiya
        `)
    ).toThrow(greedyRemovedMessage);
  });

  test("greedy options error names the offending expression", () => {
    expect(() => Grammar.parseString(`item = "ab" <|> "abc" <|> "a"`)).toThrow(
      'Found: `"ab" <|> "abc" <|> "a"`'
    );
  });

  test("greedy options nested in a group or sequence are rejected too", () => {
    expect(() => Grammar.parseString(`item = "x" + ("ab" <|> "abc")`)).toThrow(
      greedyRemovedMessage
    );
    expect(() => patterns`item = ("ab" <|> "abc")+`).toThrow(greedyRemovedMessage);
  });

  test("greedy options error is not a generic syntax error", () => {
    let message = "";
    try {
      Grammar.parseString(`item = "ab" <|> "abc"`);
    } catch (e) {
      message = (e         ).message;
    }
    expect(message).toContain(greedyRemovedMessage);
    expect(message).not.toContain("Hint:");
    expect(message).toContain("at line 1, column 8");
  });

  test("`<|>` inside a literal is just text", () => {
    const { arrow } = patterns`arrow = "<|>"`;
    expect(arrow.exec("<|>").ast?.value).toBe("<|>");
  });

  // (`hi | hiya` would now be rejected as a shadowed literal.)
  test("options picks the first match, in order", () => {
    const { greeting } = patterns`
            hi = "hi"
            hiya = "hiya"
            greeting = hiya | hi
        `;
    expect(greeting.exec("hiya").ast?.value).toBe("hiya");
    expect(greeting.exec("hi").ast?.value).toBe("hi");
  });

  // --- Default Params ---

  test("default param resolves to locally defined pattern when no param supplied", () => {
    const p = Grammar.parseString(`
            use params { value = default-value }
            default-value = "DefaultValue"
            alias = value
        `);
    expect(p["alias"].exec("DefaultValue").ast?.value).toBe("DefaultValue");
  });

  test("default param is overridden when param is supplied", () => {
    const p = Grammar.parseString(
      `
            use params { value = default-value }
            default-value = "DefaultValue"
            alias = value
        `,
      { params: [new Literal("value", "Supplied")] }
    );
    expect(p["alias"].exec("Supplied").ast?.value).toBe("Supplied");
  });

  test("default param resolves to imported pattern", async () => {
    const p = await Grammar.parse(
      `
            import { my-value as default-value } from "r1"
            use params { value = default-value }
            alias = value
        `,
      {
        resolveImport: () =>
          Promise.resolve({ expression: `my-value = "MyValue"`, resource: "r1" }),
      }
    );
    expect(p["alias"].exec("MyValue").ast?.value).toBe("MyValue");
  });

  test("use params without default declares param available from options", () => {
    const p = Grammar.parseString(
      `
            use params { value }
            alias = value
        `,
      { params: [new Literal("value", "Provided")] }
    );
    expect(p["alias"].exec("Provided").ast?.value).toBe("Provided");
  });

  // --- Expression Precedence ---

  test("expression respects declaration-order precedence for arithmetic", () => {
    const { expr } = patterns`
            number = /\\d+/
            mul-expr = expr + " * " + expr
            add-expr = expr + " + " + expr
            expr = mul-expr | add-expr | number
        `;
    const result = expr.exec("1 + 2 * 3");
    expect(result.ast?.value).toBe("1 + 2 * 3");
    expect(result.ast .value).toContain("+");
  });

  test("expression handles prefix operator", () => {
    const { expr } = patterns`
            number = /\\d+/
            neg = "-" + expr
            expr = neg | number
        `;
    expect(expr.exec("-42").ast?.value).toBe("-42");
  });

  test("expression handles postfix operator", () => {
    const { expr } = patterns`
            number = /\\d+/
            factorial = expr + "!"
            expr = factorial | number
        `;
    expect(expr.exec("5!").ast?.value).toBe("5!");
  });

  // --- Forward References ---

  test("forward reference resolves pattern defined later", () => {
    const p = Grammar.parseString(`
            greeting = "Hello " + name
            name = /\\w+/
        `);
    expect(p["greeting"].exec("Hello World").ast?.value).toBe("Hello World");
  });

  // --- Import Errors ---

  test("duplicate import name throws", async () => {
    await expect(
      Grammar.parse(
        `
            import { value } from "r1"
            import { value } from "r2"
        `,
        {
          resolveImport: r => Promise.resolve({ expression: `value = "A"`, resource: r }),
        }
      )
    ).rejects.toThrow("already used");
  });

  test("importing nonexistent pattern throws", async () => {
    await expect(
      Grammar.parse(
        `
            import { nonexistent } from "r1"
        `,
        {
          resolveImport: () =>
            Promise.resolve({ expression: `value = "A"`, resource: "r1" }),
        }
      )
    ).rejects.toThrow("Couldn't find pattern");
  });

  // --- Nested Groups ---

  test("nested parenthesized groups unwrap correctly", () => {
    const p = Grammar.parseString(`
            word = "hello"
            inner = ((word))
        `);
    expect(p["inner"].exec("hello").ast?.value).toBe("hello");
  });

  // --- Alias Chain ---

  test("alias chain resolves through multiple levels", () => {
    const p = Grammar.parseString(`
            original = "value"
            alias1 = original
            alias2 = alias1
        `);
    expect(p["alias2"].exec("value").ast?.value).toBe("value");
  });

  // --- Multiple Decorators ---

  test("multiple decorators applied to same pattern", () => {
    const callOrder           = [];
    const customPatterns = createPatternsTemplate({
      decorators: {
        first: pattern => {
          callOrder.push("first:" + pattern.name);
        },
        second: pattern => {
          callOrder.push("second:" + pattern.name);
        },
      },
    });
    customPatterns`
            @first
            @second
            name = /\\w+/
        `;
    expect(callOrder).toContain("first:name");
    expect(callOrder).toContain("second:name");
  });

  // --- Decorator JSON Arg Types ---

  test("decorator receives number argument", () => {
    let received                       ;
    const cp = createPatternsTemplate({
      decorators: {
        config: (_p, arg) => {
          received = arg;
        },
      },
    });
    cp`@config(42)\nname = /\\w+/`;
    expect(received).toBe(42);
  });

  test("decorator receives nested array argument", () => {
    let received                       ;
    const cp = createPatternsTemplate({
      decorators: {
        config: (_p, arg) => {
          received = arg;
        },
      },
    });
    cp`@config([1, 2, [3]])\nname = /\\w+/`;
    expect(received).toEqual([1, 2, [3]]);
  });

  test("decorator receives null argument", () => {
    let received                        = "not-null";
    const cp = createPatternsTemplate({
      decorators: {
        config: (_p, arg) => {
          received = arg;
        },
      },
    });
    cp`@config(null)\nname = /\\w+/`;
    expect(received).toBeNull();
  });

  test("decorator receives string argument", () => {
    let received                       ;
    const cp = createPatternsTemplate({
      decorators: {
        config: (_p, arg) => {
          received = arg;
        },
      },
    });
    cp`@config("hello")\nname = /\\w+/`;
    expect(received).toBe("hello");
  });

  // --- Patterns Template ---

  test("patterns template converts kebab-case to camelCase", () => {
    const result = patterns`my-long-name = "hello"`;
    expect(result["myLongName"]).toBeDefined();
    expect(result["myLongName"].exec("hello").ast?.value).toBe("hello");
  });

  // --- Edge Case Grammars ---

  test("empty grammar throws", () => {
    expect(() => Grammar.parseString("")).toThrow("[Invalid Grammar]");
  });

  test("grammar with only comments produces no patterns", () => {
    const p = Grammar.parseString("# just a comment");
    expect(Object.keys(p)).toHaveLength(0);
  });

  test("grammar with syntax version and no patterns", () => {
    const p = Grammar.parseString("syntax 1.0");
    expect(Object.keys(p)).toHaveLength(0);
  });

  // --- Inline Anonymous Patterns ---

  test("repeat with inline literal pattern", () => {
    const p = Grammar.parseString(`items = ("item")+`);
    expect(p["items"].exec("itemitemitem").ast?.value).toBe("itemitemitem");
  });

  test("repeat with inline regex pattern", () => {
    const p = Grammar.parseString(`digits = (/\\d/)+`);
    expect(p["digits"].exec("123").ast?.value).toBe("123");
  });

  test("inline literals use their value as the pattern name", () => {
    const p = Grammar.parseString(`greeting = "Hello" + " " + /\\w+/`);
    const ast = p["greeting"].exec("Hello World").ast ;
    expect(ast.children[0].name).toBe("Hello");
    expect(ast.children[1].name).toBe(" ");
    expect(ast.children[2].name).toBe("\\w+");
  });

  // --- Flattening ---

  test("five-way sequence flattens correctly", () => {
    const p = Grammar.parseString(`result = "a" + "b" + "c" + "d" + "e"`);
    expect(p["result"].exec("abcde").ast?.value).toBe("abcde");
  });

  test("four-way options all reachable", () => {
    const p = Grammar.parseString(`letter = "a" | "b" | "c" | "d"`);
    expect(p["letter"].exec("a").ast?.value).toBe("a");
    expect(p["letter"].exec("d").ast?.value).toBe("d");
  });

  // --- Complex Composition ---

  test("sequence mixing references, optionals, nots, and inline patterns", () => {
    const p = Grammar.parseString(`
            keyword = "let"
            ws = /\\s+/
            ident = /[a-z]+/
            eq = "="
            digit = /\\d+/
            let-stmt = keyword + ws + !digit + ident + ws? + eq + ws? + digit
        `);
    expect(p["let-stmt"].exec("let x=42").ast?.value).toBe("let x=42");
    expect(p["let-stmt"].exec("let 9=42").ast).toBeNull();
  });

  test("recursive list with nested expressions", () => {
    const p = Grammar.parseString(`
            digit = /\\d+/
            comma = ", "
            open = "("
            close = ")"
            item = digit | list
            items = (item, comma trim)+
            list = open + items + close
        `);
    expect(p["list"].exec("(1, (2, 3), 4)").ast?.value).toBe("(1, (2, 3), 4)");
  });

  // --- AST Structure ---

  test("sequence children reflect pattern names in AST", () => {
    const { fullName } = patterns`
            first = /[A-Z][a-z]+/
            space = " "
            last = /[A-Z][a-z]+/
            full-name = first + space + last
        `;
    const ast = fullName.exec("John Doe").ast ;
    expect(ast.children.length).toBe(3);
    expect(ast.children[0].name).toBe("first");
    expect(ast.children[0].value).toBe("John");
    expect(ast.children[2].name).toBe("last");
    expect(ast.children[2].value).toBe("Doe");
  });

  test("repeat AST contains repeated children", () => {
    const { digits } = patterns`
            digit = /\\d/
            digits = (digit)+
        `;
    const ast = digits.exec("123").ast ;
    expect(ast.children.length).toBe(3);
    expect(ast.children[0].value).toBe("1");
    expect(ast.children[1].value).toBe("2");
    expect(ast.children[2].value).toBe("3");
  });

  test("options AST returns winning alternative directly", () => {
    const { choice } = patterns`
            a = "aaa"
            b = "bbb"
            choice = a | b
        `;
    const ast = choice.exec("bbb").ast ;
    expect(ast.name).toBe("b");
    expect(ast.value).toBe("bbb");
  });

  test("expression AST wraps in expression node", () => {
    const { expr } = patterns`
            num = /\\d+/
            add = expr + " + " + expr
            expr = add | num
        `;
    expect(expr.exec("1 + 2").ast .type).toBe("expression");
  });

  // --- Multiple Names in One Import ---

  test("import multiple patterns from same resource", async () => {
    const p = await Grammar.parse(
      `
            import { first, last } from "names.cpat"
            full = first + " " + last
        `,
      {
        resolveImport: () =>
          Promise.resolve({
            expression: `first = "John"\nlast = "Doe"`,
            resource: "names.cpat",
          }),
      }
    );
    expect(p["full"].exec("John Doe").ast?.value).toBe("John Doe");
  });

  test("import multiple patterns with alias from same resource", async () => {
    const p = await Grammar.parse(
      `
            import { first, last as surname } from "names.cpat"
            full = first + " " + surname
        `,
      {
        resolveImport: () =>
          Promise.resolve({
            expression: `first = "John"\nlast = "Doe"`,
            resource: "names.cpat",
          }),
      }
    );
    expect(p["full"].exec("John Doe").ast?.value).toBe("John Doe");
  });

  // --- Repeat with Inline Complex Pattern ---

  test("repeat with inline sequence as repeated item", () => {
    const p = Grammar.parseString(`
            pairs = ("(" + /\\w+/ + ")")+
        `);
    expect(p["pairs"].exec("(a)(b)(c)").ast?.value).toBe("(a)(b)(c)");
  });

  test("repeat with inline options as repeated item", () => {
    const p = Grammar.parseString(`
            bits = ("0" | "1")+
        `);
    expect(p["bits"].exec("1010").ast?.value).toBe("1010");
  });

  // --- TakeUntil with Regex Terminator ---

  test("take until with regex terminator", () => {
    const { content } = patterns`
            content = ?->| /[<]/
        `;
    const result = content.parse(new Cursor("hello world<"));
    expect(result?.value).toBe("hello world");
  });

  // --- Inline Not and Optional with Literals ---

  test("inline not with literal in sequence", () => {
    const p = Grammar.parseString(`
            word = /\\w+/
            not-hello = !"hello" + word
        `);
    expect(p["not-hello"].exec("world").ast?.value).toBe("world");
    expect(p["not-hello"].exec("hello").ast).toBeNull();
  });

  test("inline optional with literal in sequence", () => {
    const p = Grammar.parseString(`
            greeting = "hello" + " world"?
        `);
    expect(p["greeting"].exec("hello world").ast?.value).toBe("hello world");
    expect(p["greeting"].exec("hello").ast?.value).toBe("hello");
  });

  // --- Syntax Version Combined with Patterns ---

  test("syntax version followed by patterns parses correctly", () => {
    const p = Grammar.parseString(`
            syntax 1.0
            name = "hello"
        `);
    expect(p["name"].exec("hello").ast?.value).toBe("hello");
  });

  // --- Empty String Literal ---

  test("empty string literal throws", () => {
    // Deviation (DESIGN §18): a pattern that can't be constructed is
    // reported in cpat-rs's words, which name the pattern; v12 says "Value
    // Cannot be empty."
    expect(() => Grammar.parseString(`empty = ""`)).toThrow(
      "literal `empty` is empty; a literal must match at least one character"
    );
  });

  // --- Block Pattern ---

  test("Block with wildcard content", () => {
    const p = Grammar.parseString(`
            braces = ["{"] ... ["}"]
        `);
    const result = p["braces"].exec("{}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .type).toBe("block");
  });

  test("Block without content requires ...", () => {
    expect(() =>
      Grammar.parseString(`
            braces = ["{"] ["}"]
        `)
    ).toThrow();
  });

  test("Block with content pattern", () => {
    const p = Grammar.parseString(`
            content = /[^{}]+/
            braces = ["{"] content ["}"]
        `);
    const result = p["braces"].exec("{hello}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("{hello}");
  });

  test("Block with multiple content patterns", () => {
    const p = Grammar.parseString(`
            key = /[a-z]+/
            value = /[a-z]+/
            pair = ["{"] key + ":" + value ["}"]
        `);
    const result = p["pair"].exec("{foo:bar}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("{foo:bar}");
  });

  test("Block handles nesting", () => {
    const p = Grammar.parseString(`
            braces = ["{"] ... ["}"]
        `);
    const cursor = new Cursor("{ { } }");
    const ctx = p["braces"]           ;
    const result = ctx.find(p => p.type === "block") .parse(cursor);
    expect(result).not.toBeNull();
    expect(result .firstIndex).toBe(0);
    expect(result .lastIndex).toBe(6);
  });

  test("Block with inline literal content", () => {
    const p = Grammar.parseString(`
            parens = ["("] /[^()]+/ [")"]
        `);
    const result = p["parens"].exec("(hello)");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("(hello)");
  });

  test("Block wildcard matches any content", () => {
    const p = Grammar.parseString(`
            braces = ["{"] ... ["}"]
        `);
    const result = p["braces"].exec("{anything goes here}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("{anything goes here}");
  });

  test("Block with inline regex content", () => {
    const p = Grammar.parseString(`
            braces = ["{"] /[^{}]*/ ["}"]
        `);
    const result = p["braces"].exec("{hello world}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("{hello world}");
  });

  test("Block with inline literal content", () => {
    const p = Grammar.parseString(`
            parens = ["("] "hello" [")"]
        `);
    const result = p["parens"].exec("(hello)");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("(hello)");
  });

  test("Block with pattern reference content", () => {
    const p = Grammar.parseString(`
            word = /[a-z]+/
            braces = ["{"] word ["}"]
        `);
    const result = p["braces"].exec("{hello}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("{hello}");
  });

  test("Block with repeat content", () => {
    const p = Grammar.parseString(`
            item = /[a-z]+/
            braces = ["{"] (item, ",")+ ["}"]
        `);
    const result = p["braces"].exec("{a,b,c}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("{a,b,c}");
  });

  test("Block with grouped options content", () => {
    const p = Grammar.parseString(`
            braces = ["{"] ("yes" | "no") ["}"]
        `);
    expect(p["braces"].exec("{yes}").ast?.value).toBe("{yes}");
    expect(p["braces"].exec("{no}").ast?.value).toBe("{no}");
  });

  test("Block with inline sequence content", () => {
    const p = Grammar.parseString(`
            key = /[a-z]+/
            value = /[0-9]+/
            braces = ["{"] key + "=" + value ["}"]
        `);
    const result = p["braces"].exec("{x=42}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("{x=42}");
  });

  test("Block with three-part inline sequence content", () => {
    const p = Grammar.parseString(`
            a = "a"
            b = "b"
            c = "c"
            braces = ["{"] a + b + c ["}"]
        `);
    const result = p["braces"].exec("{abc}");
    expect(result.ast).not.toBeNull();
    expect(result.ast .value).toBe("{abc}");
  });

  test("Block with ungrouped options content", () => {
    const p = Grammar.parseString(`
            braces = ["{"] "yes" | "no" ["}"]
        `);
    expect(p["braces"].exec("{yes}").ast?.value).toBe("{yes}");
    expect(p["braces"].exec("{no}").ast?.value).toBe("{no}");
  });

  // --- Import Without Resolver ---

  test("import without resolver (sync) throws", () => {
    expect(() =>
      Grammar.parseString(`
            import { value } from "file.cpat"
            name = value
        `)
    ).toThrow("No import resolver supplied.");
  });

  test("import without resolver (async) throws", async () => {
    await expect(
      Grammar.parse(`
            import { value } from "file.cpat"
            name = value
        `)
    ).rejects.toThrow("No import resolver supplied.");
  });

  // --- Sync Import Error Wrapping ---

  test("sync import wraps error from bad imported grammar", () => {
    expect(() =>
      Grammar.parseString(
        `
            import { value } from "bad.cpat"
            name = value
        `,
        {
          resolveImportSync: () => ({
            expression: "///bad grammar///",
            resource: "bad.cpat",
          }),
        }
      )
    ).toThrow(/Failed loading expression from: "bad\.cpat"/);
  });

  // --- Async Import Error Wrapping ---

  test("async import wraps error from bad imported grammar", async () => {
    await expect(
      Grammar.parse(
        `
            import { value } from "bad.cpat"
            name = value
        `,
        {
          resolveImport: () =>
            Promise.resolve({
              expression: "///bad grammar///",
              resource: "bad.cpat",
            }),
        }
      )
    ).rejects.toThrow(/Failed loading expression from: "bad\.cpat"/);
  });

  // --- Import Alias Errors ---

  test("duplicate import alias throws", async () => {
    await expect(
      Grammar.parse(
        `
            import { a as alias } from "r1"
            import { b as alias } from "r2"
        `,
        {
          resolveImport: r =>
            Promise.resolve({
              expression: r === "r1" ? `a = "A"` : `b = "B"`,
              resource: r,
            }),
        }
      )
    ).rejects.toThrow("already used");
  });

  test("importing nonexistent pattern via alias throws", async () => {
    await expect(
      Grammar.parse(
        `
            import { nonexistent as alias } from "r1"
        `,
        {
          resolveImport: () =>
            Promise.resolve({
              expression: `value = "A"`,
              resource: "r1",
            }),
        }
      )
    ).rejects.toThrow("Couldn't find pattern");
  });

  // --- Export Pattern (sync path) ---

  test("export pattern promotes previously defined pattern", () => {
    const p = Grammar.parseString(
      `
            import { value } from "r1"
            value
        `,
      {
        resolveImportSync: () => ({
          expression: `value = "Hello"`,
          resource: "r1",
        }),
      }
    );
    expect(p["value"].exec("Hello").ast?.value).toBe("Hello");
  });
});
