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

/**
 * A pattern whose matching is your function: `match(text, pos)` answers the
 * end of a match starting at `pos` (exclusive), or `null` for none. Both
 * are UTF-16 offsets, as JS strings count. Not in v12, whose custom
 * patterns implement the whole `Pattern` contract; here a parse runs in
 * wasm, so the contract is one call per attempt (DESIGN §9).
 *
 * Answering `pos` itself matches nothing and succeeds (as an absent
 * optional does). Throwing, or answering anything but `null` or a whole
 * number from `pos` to `text.length` that doesn't split a surrogate pair,
 * stops the parse: `exec` throws `custom pattern \`name\` failed: …`.
 */
export class CustomPattern extends PatternBase {
  constructor(name, match) {
    if (typeof match !== "function") {
      throw new TypeError(`CustomPattern \`${name}\`: match must be a function (text, pos) => end | null, got ${match}`);
    }
    super(WasmPattern.custom(name, new CustomHost(match)));
  }

  setTokens(tokens) {
    setTokens(this, tokens);
  }
}

// What wasm calls (src/custom.rs): the text once per parse, then a
// position per attempt. Answers -1 for no match, and turns a throw or a
// bad answer into the message the parse stops with.
class CustomHost {
  constructor(match) {
    this._match = match;
    this._text = "";
  }

  setText(text) {
    this._text = text;
  }

  match(pos) {
    let end;
    try {
      end = this._match(this._text, pos);
    } catch (thrown) {
      throw thrown instanceof Error ? thrown.message : String(thrown);
    }
    if (end == null) {
      return -1;
    }
    if (!Number.isInteger(end) || end < pos || end > this._text.length || splitsPair(this._text, end)) {
      throw `answered ${typeof end === "string" ? JSON.stringify(end) : String(end)} at ${pos}; return null, or an end from ${pos} to ${this._text.length} (the text's length) that doesn't split a surrogate pair`;
    }
    return end;
  }
}

function splitsPair(text, at) {
  const code = text.charCodeAt(at);
  return at > 0 && at < text.length && code >= 0xdc00 && code <= 0xdfff;
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
registerPatternClass("custom", CustomPattern);
