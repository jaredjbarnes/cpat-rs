// The pattern classes, with v12's constructor signatures. Each builds its
// pattern in wasm; a composite takes copies of its children (`owned`), as
// v12's constructors clone theirs.

import { WasmPattern } from "./wasm.mjs";
import { PatternBase, owned, registerPatternClass, wrap } from "./pattern.mjs";

export class Literal extends PatternBase {
  constructor(name, value) {
    super(WasmPattern.literal(name, value));
  }

  get token() {
    return this._root.tokenAt(this._path);
  }
}

export class Regex extends PatternBase {
  constructor(name, regex) {
    super(WasmPattern.regex(name, regex));
  }

  get regex() {
    return this._root.regexAt(this._path);
  }

  setTokens(tokens) {
    setTokens(this, tokens);
  }
}

export class Sequence extends PatternBase {
  constructor(name, sequence) {
    super(WasmPattern.sequence(name, sequence.map(owned)));
  }
}

/** v12's choice: ordered, by first character, or by precedence when some
 * alternatives are operators. One pattern in cpat (DESIGN §8). */
export class Expression extends PatternBase {
  constructor(name, patterns) {
    super(WasmPattern.choice(name, patterns.map(owned)));
  }
}

export class Optional extends PatternBase {
  constructor(name, pattern) {
    super(WasmPattern.optional(name, owned(pattern)));
  }
}

export class Not extends PatternBase {
  constructor(name, pattern) {
    super(WasmPattern.not(name, owned(pattern)));
  }
}

export class Cut extends PatternBase {
  constructor(name = "cut") {
    super(WasmPattern.cut(name));
  }
}

/** `new Repeat(name, pattern, { min, max, divider, trimDivider })`. Its
 * `type` is "infinite-repeat" or "finite-repeat", as v12's facade reports
 * its inner repeat's. */
export class Repeat extends PatternBase {
  constructor(name, pattern, options = {}) {
    const { divider, ...rest } = options;
    super(
      WasmPattern.repeat(name, owned(pattern), {
        ...rest,
        divider: divider == null ? undefined : owned(divider),
      })
    );
  }
}

export class Reference extends PatternBase {
  constructor(name, referencePatternName) {
    super(WasmPattern.reference(name, referencePatternName));
  }
}

export class Context extends PatternBase {
  constructor(name, pattern, context = []) {
    super(WasmPattern.context(name, owned(pattern), context.map(owned)));
  }

  /** The definitions this context puts in scope, by name (a new object
   * each call). */
  getPatternsWithinContext() {
    const patterns = {};
    for (const pattern of this._root.patternsWithinContextAt(this._path)) {
      const wrapped = wrap(pattern);
      patterns[wrapped.name] = wrapped;
    }
    return patterns;
  }

  getPatternWithinContext(name) {
    if (this.name === name) {
      return this;
    }
    return this.getPatternsWithinContext()[name] ?? null;
  }
}

export class RightAssociated extends PatternBase {
  constructor(pattern) {
    super(WasmPattern.rightAssociated(owned(pattern)));
  }
}

export class TakeUntil extends PatternBase {
  constructor(name, terminatingPattern) {
    super(WasmPattern.takeUntil(name, owned(terminatingPattern)));
  }

  setTokens(tokens) {
    setTokens(this, tokens);
  }
}

export class Block extends PatternBase {
  constructor(name, openPattern, contentPattern, closePattern) {
    super(
      WasmPattern.block(
        name,
        owned(openPattern),
        contentPattern == null ? undefined : owned(contentPattern),
        owned(closePattern)
      )
    );
  }
}

// Tokens are set on a pattern you hold: one built here, or one a grammar
// or decorator handed you. A child reached through `children` is a place in
// its parent, which wasm doesn't change in place (DESIGN §22).
function setTokens(pattern, tokens) {
  if (pattern._path.length > 0) {
    throw new Error(
      `\`${pattern.name}\` is a child of another pattern; set its tokens before building the parent, or with @tokens in a grammar`
    );
  }
  pattern._root.setTokens(tokens);
}

/** v12's capability check: a pattern takes tokens if it has `setTokens`. */
export function isTokenizablePattern(pattern) {
  return typeof pattern?.setTokens === "function";
}

registerPatternClass("literal", Literal);
registerPatternClass("regex", Regex);
registerPatternClass("sequence", Sequence);
registerPatternClass("expression", Expression);
registerPatternClass("optional", Optional);
registerPatternClass("not", Not);
registerPatternClass("cut", Cut);
registerPatternClass("infinite-repeat", Repeat);
registerPatternClass("finite-repeat", Repeat);
registerPatternClass("reference", Reference);
registerPatternClass("context", Context);
registerPatternClass("right-associated", RightAssociated);
registerPatternClass("take-until", TakeUntil);
registerPatternClass("block", Block);
