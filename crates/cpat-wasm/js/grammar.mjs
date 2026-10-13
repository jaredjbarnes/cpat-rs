// v12's `Grammar`: `.cpat` text in, patterns by rule name out.
//
// The build runs in wasm and is synchronous (DESIGN §19). `parse` and
// `import` are asynchronous only because the user's `resolveImport` is:
// they fetch every grammar the text imports first, depth first in the
// order the build will ask for them (wasm's `importsOf` lists a text's
// imports), and then build with a resolver that answers from what was
// fetched (DESIGN §22).

import { buildGrammar, importsOf } from "./wasm.mjs";
import { PatternBase, owned, wrap } from "./pattern.mjs";

const NO_RESOLVER = "No import resolver supplied.";

function messageOf(thrown) {
  return thrown instanceof Error ? thrown.message : String(thrown);
}

function noResolver() {
  throw new Error(NO_RESOLVER);
}

export class Grammar {
  constructor(options = {}) {
    this._params = options.params || [];
    this._originResource = options.originResource || null;
    this._decorators = options.decorators || {};
    this._resolveImport = options.resolveImport ?? null;
    this._resolveImportSync = options.resolveImportSync ?? null;
  }

  /** The grammar `resolveImport` finds at `path`, built with its own
   * imports resolved from there. */
  async import(path) {
    const resolve = this._resolveImport ?? noResolver;
    const grammarFile = await resolve(path, null);
    const grammar = new Grammar({
      resolveImport: this._resolveImport,
      originResource: grammarFile.resource,
      params: this._params,
      decorators: this._decorators,
    });
    return grammar.parse(grammarFile.expression);
  }

  async parse(expression) {
    const fetched = new Fetched();
    await fetched.fetch(expression, this._originResource, this._resolveImport ?? noResolver);
    return this._build(expression, (resource, origin) => fetched.take(resource, origin));
  }

  parseString(expression) {
    return this._build(expression, this._resolveImportSync ?? noResolver);
  }

  static parse(expression, options) {
    return new Grammar(options).parse(expression);
  }

  static import(path, options) {
    return new Grammar(options).import(path);
  }

  static parseString(expression, options) {
    return new Grammar(options).parseString(expression);
  }

  // Wasm calls back for each import and each of the user's decorators.
  // Whatever those throw goes back as its message, which the build reports
  // as v12 would (unwrapped, or inside "Failed loading expression from").
  _build(expression, resolve) {
    const decorators = this._decorators;
    const host = {
      resolve(resource, origin) {
        try {
          const file = resolve(resource, origin ?? null);
          return [file.resource, file.expression];
        } catch (thrown) {
          throw messageOf(thrown);
        }
      },
      decorate(name, pattern, arg) {
        const decorated = wrap(pattern);
        let returned;
        try {
          returned = decorators[name](decorated, arg === undefined ? undefined : JSON.parse(arg));
        } catch (thrown) {
          throw messageOf(thrown);
        }
        return kept(decorated, returned);
      },
    };

    const record = buildGrammar(
      expression,
      this._originResource ?? undefined,
      this._params.map(owned),
      Object.keys(decorators),
      host
    );
    const names = record.names;
    const patterns = record.takePatterns();
    const result = {};
    names.forEach((name, i) => {
      result[name] = wrap(patterns[i]);
    });
    return result;
  }
}

/**
 * What a decorator leaves the rule with: the pattern it returned in place of
 * the one it was handed (a cpat addition, DESIGN §9: a `CustomPattern`,
 * say), or else the one it was handed, changed in place as v12's decorators
 * change it. Any other return value is ignored, as v12 ignores it: a
 * decorator written `(p, arg) => seen.push(arg)` returns a number. A copy
 * goes back either way: wasm takes what it is handed, and the decorator may
 * have kept the object.
 */
function kept(decorated, returned) {
  return returned instanceof PatternBase ? owned(returned) : decorated._root.clone();
}

/**
 * Grammars fetched ahead of a build, by resource and origin. Each import is
 * asked for once per statement, as v12 asks, and the answers are handed out
 * in the order they were fetched, which is the order the build asks in. An
 * answer that was a failure stops the fetching (v12 would have stopped
 * there) and is thrown when the build reaches it.
 */
class Fetched {
  constructor() {
    this._answers = new Map();
  }

  async fetch(expression, origin, resolveImport) {
    const resources = importsOf(expression);
    if (resources == null) {
      // The text doesn't build; the build will say why.
      return false;
    }
    for (const resource of resources) {
      let answer;
      try {
        answer = { file: await resolveImport(resource, origin) };
      } catch (error) {
        answer = { error };
      }
      this._queue(resource, origin).push(answer);
      if (answer.error !== undefined) {
        return false;
      }
      const file = answer.file;
      if (!(await this.fetch(file.expression, file.resource || null, resolveImport))) {
        return false;
      }
    }
    return true;
  }

  /** The next answer for `resource` from `origin`. The build asks for
   * exactly what was fetched, in the same order, and stops where the
   * fetching stopped, so every question has its answer. */
  take(resource, origin) {
    const queue = this._answers.get(key(resource, origin));
    if (queue == null || queue.length === 0) {
      throw new Error(`"${resource}" was not fetched before the build asked for it`);
    }
    const answer = queue.shift();
    if (answer.error !== undefined) {
      throw answer.error;
    }
    return answer.file;
  }

  _queue(resource, origin) {
    const k = key(resource, origin);
    if (!this._answers.has(k)) {
      this._answers.set(k, []);
    }
    return this._answers.get(k);
  }
}

function key(resource, origin) {
  return JSON.stringify([resource, origin ?? null]);
}
