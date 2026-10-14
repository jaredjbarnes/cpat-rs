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

  // v12's classified alternatives, as places among `children` (the same
  // objects): operands, and operators without their self-references
  // (`<name>-prefix`, `-postfix`, `-delimiter`).

  get atomPatterns() {
    return this._childrenWithRole("atom");
  }

  get prefixPatterns() {
    return this._childrenWithRole("prefix");
  }

  get postfixPatterns() {
    return this._childrenWithRole("postfix");
  }

  get infixPatterns() {
    return this._childrenWithRole("infix");
  }

  /** @deprecated v12's old name for `infixPatterns` (the same array). */
  get binaryPatterns() {
    return this.infixPatterns;
  }

  /** The alternatives as given, operators with their self-references: each
   * a copy of its own, since an operator's alternative is no place among
   * the classified children. */
  get originalPatterns() {
    if (this._originalPatterns === undefined) {
      this._originalPatterns = Object.freeze(this._root.originalPatternsAt(this._path).map(p => wrap(p)));
    }
    return this._originalPatterns;
  }

  /** v12 classifies on the first parse or on `build()`, and throws v12's
   * `Couldn't find 'x' pattern within tree.` when an alternative is a
   * reference that names nothing. Here the accessors classify when first
   * read (DESIGN §22), so what is left is that check, made from this place
   * as a parse would resolve the names. */
  build() {
    this._root.buildAt(this._path);
  }

  _childrenWithRole(role) {
    this._byRole ??= {};
    if (this._byRole[role] === undefined) {
      const roles = this._root.choiceRolesAt(this._path) ?? [];
      this._byRole[role] = Object.freeze(this.children.filter((_, i) => roles[i] === role));
    }
    return this._byRole[role];
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

  // v12's facade answers these two for itself: what may follow its inner
  // repeat, whatever child is named (the children's parent is the inner
  // repeat, never the facade). cpat has no facade, so they are answered
  // here as v12's are.

  getTokensAfter(_childReference) {
    return this.getNextTokens();
  }

  getPatternsAfter(_childReference) {
    return this.getNextPatterns();
  }

  /** As given, as v12's facade reports it: `{ min: 0 }` says 0, though the
   * repeat parses as one or more. */
  get min() {
    return this._root.repeatBoundsAt(this._path)[0];
  }

  /** As given; `Infinity` when unbounded, as in v12. */
  get max() {
    return this._root.repeatBoundsAt(this._path)[1];
  }

  /** The pattern repeated, as given: a copy, not one of the `children`
   * (a bounded repeat has a child slot per repetition). */
  get pattern() {
    if (this._pattern === undefined) {
      this._pattern = wrap(this._root.repeatPatternAt(this._path));
    }
    return this._pattern;
  }

  /** v12's `{ ...options, min, max }`: `divider` (a copy) when there is one,
   * and `trimDivider` when it is on. A `trimDivider: false` you passed
   * isn't kept (wasm doesn't store whether it was given). */
  get options() {
    if (this._options === undefined) {
      const [min, max] = this._root.repeatBoundsAt(this._path);
      const divider = this._root.repeatDividerAt(this._path);
      const options = { min, max };
      if (divider != null) {
        options.divider = wrap(divider);
      }
      if (this._root.trimDividerAt(this._path)) {
        options.trimDivider = true;
      }
      this._options = Object.freeze(options);
    }
    return this._options;
  }
}

export class Reference extends PatternBase {
  constructor(name, referencePatternName) {
    super(WasmPattern.reference(name, referencePatternName));
  }

  /** What this reference resolves to, as a parse would resolve it here
   * (its only child, named after the reference); throws v12's error when
   * the name resolves to nothing. */
  getReferencePatternSafely() {
    const target = this.children[0];
    if (target == null) {
      throw new Error(`Couldn't find '${this._root.referenceTargetAt(this._path)}' pattern within tree.`);
    }
    return target;
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
    // Held here, not read back from `_text` after the call: the function may
    // parse again through a copy of this pattern, which shares this host and
    // calls setText with that other parse's text before we check the answer.
    const text = this._text;
    let end;
    try {
      end = this._match(text, pos);
    } catch (thrown) {
      throw thrown instanceof Error ? thrown.message : String(thrown);
    }
    if (end == null) {
      return -1;
    }
    if (!Number.isInteger(end) || end < pos || end > text.length || splitsPair(text, end)) {
      throw `answered ${typeof end === "string" ? JSON.stringify(end) : String(end)} at ${pos}; return null, or an end from ${pos} to ${text.length} (the text's length) that doesn't split a surrogate pair`;
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
