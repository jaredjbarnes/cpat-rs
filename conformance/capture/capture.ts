// Logs every (grammar, rule, input) a test suite runs through v12, by
// patching Grammar.prototype.parseString/parse and AutoComplete. A build is
// logged with what its options did, so the text and that record rebuild it:
// the params (literals and regexes, by value), the names of the custom
// decorators, the origin resource, and every answer the import resolvers
// gave (conformance/lib/replay.mjs answers from that record). A build whose
// params can't be written down (a hand-made Sequence, say) is not logged.
import { createHash } from "node:crypto";
import { appendFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

type Tag = { g: string; rule: string };
type Call = { resource: string; origin: string | null; file?: { resource: string; expression: string }; error?: string };
type Options = {
  entry: "parseString" | "parse";
  originResource?: string;
  params?: object[];
  decorators?: string[];
  resolveImport?: Call[];
  resolveImportSync?: Call[];
};

// v12's resolvers when the caller gives none: they throw, and replaying
// without a resolver throws the same.
const DEFAULT_RESOLVERS = new Set(["defaultImportResolver", "defaultImportResolverSync"]);
const RESOLVERS = [
  ["_resolveImport", "resolveImport", true],
  ["_resolveImportSync", "resolveImportSync", false],
] as const;

const messageOf = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** A param by value, or null when it is no literal or regex. */
function describeParam(p: any): object | null {
  if (p.type === "literal") return { type: "literal", name: p.name, value: p.token };
  if (p.type === "regex") return { type: "regex", name: p.name, regex: p.regex };
  return null;
}

/** `resolve`, writing each answer (or throw) into `calls`. */
function recording(resolve: Function, calls: Call[], async: boolean) {
  const answered = (resource: string, origin: string | null, file: any) => {
    calls.push({ resource, origin, file: { resource: file.resource, expression: file.expression } });
    return file;
  };
  const failed = (resource: string, origin: string | null, e: unknown) => {
    calls.push({ resource, origin, error: messageOf(e) });
    throw e;
  };
  return function (resource: string, origin: string | null) {
    try {
      const answer = resolve(resource, origin);
      if (!async) return answered(resource, origin, answer);
      return Promise.resolve(answer).then(
        file => answered(resource, origin, file),
        e => failed(resource, origin, e)
      );
    } catch (e) {
      return failed(resource, origin, e);
    }
  };
}

export function install(Grammar: any, AutoComplete: any, source: string, label: (text: string) => string | null, origin: () => string) {
  const outDir = process.env.CAPTURE_OUT;
  if (!outDir) throw new Error("CAPTURE_OUT not set");
  mkdirSync(outDir, { recursive: true });
  const file = join(outDir, `${source}-${process.pid}-${Math.random().toString(36).slice(2)}.jsonl`);
  const write = (rec: object) => appendFileSync(file, JSON.stringify(rec) + "\n");
  const tags = new WeakMap<object, Tag>();
  const seenGrammars = new Set<string>();

  /** The options of a top-level build, with its resolvers swapped for
   * recording ones; null for part of another build or an unwritable one. */
  const begin = (g: any, entry: Options["entry"]): { options: Options; restore: () => void } | null => {
    if (g._sharedAnonymousNames != null) return null;
    const params = (g._params ?? []).map(describeParam);
    if (params.some((p: object | null) => p == null)) return null;
    const options: Options = { entry };
    if (g._originResource != null) options.originResource = g._originResource;
    if (params.length > 0) options.params = params;
    const decorators = Object.keys(g._parseContext?.decorators ?? {}).filter(k => k !== "tokens");
    if (decorators.length > 0) options.decorators = decorators;
    const originals: [string, Function][] = [];
    for (const [field, key, async] of RESOLVERS) {
      const resolve = g[field];
      if (DEFAULT_RESOLVERS.has(resolve?.name)) continue;
      const calls: Call[] = [];
      options[key] = calls;
      originals.push([field, resolve]);
      g[field] = recording(resolve, calls, async);
    }
    return { options, restore: () => originals.forEach(([field, resolve]) => (g[field] = resolve)) };
  };

  const isPlain = (options: Options) => Object.keys(options).length === 1 && options.entry === "parseString";

  /** Logs the build and tags its rules; plain builds keep the id the text
   * alone gives, so the corpus of plain grammars stays as it was. */
  const finish = (expression: string, options: Options, error: string | null, patterns: Record<string, any> | null) => {
    const key = isPlain(options) ? expression : JSON.stringify({ expression, options });
    const g = createHash("sha1").update(key).digest("hex").slice(0, 16);
    if (!seenGrammars.has(g)) {
      seenGrammars.add(g);
      const record = { t: "grammar", source, g, file: label(expression), text: expression, error, origin: origin() };
      write(isPlain(options) ? record : { ...record, options });
    }
    if (patterns != null) tagRules(g, patterns);
  };

  const tagRules = (g: string, patterns: Record<string, any>) => {
    for (const [rule, pattern] of Object.entries(patterns)) {
      tags.set(pattern, { g, rule });
      const exec = pattern.exec;
      const test = pattern.test;
      pattern.exec = function (text: string, record?: boolean) {
        write({ t: "exec", source, g, rule, input: text, origin: origin() });
        return exec.call(this, text, record);
      };
      pattern.test = function (text: string, record?: boolean) {
        write({ t: "exec", source, g, rule, input: text, origin: origin() });
        return test.call(this, text, record);
      };
    }
  };

  const parseString = Grammar.prototype.parseString;
  Grammar.prototype.parseString = function (expression: string) {
    const started = begin(this, "parseString");
    if (started == null) return parseString.call(this, expression);
    try {
      const patterns = parseString.call(this, expression);
      finish(expression, started.options, null, patterns);
      return patterns;
    } catch (e) {
      finish(expression, started.options, messageOf(e), null);
      throw e;
    } finally {
      started.restore();
    }
  };

  const parse = Grammar.prototype.parse;
  Grammar.prototype.parse = async function (expression: string) {
    const started = begin(this, "parse");
    if (started == null) return parse.call(this, expression);
    try {
      const patterns = await parse.call(this, expression);
      finish(expression, started.options, null, patterns);
      return patterns;
    } catch (e) {
      finish(expression, started.options, messageOf(e), null);
      throw e;
    } finally {
      started.restore();
    }
  };

  const suggestFor = AutoComplete.prototype.suggestFor;
  AutoComplete.prototype.suggestFor = function (text: string) {
    const tag = tags.get(this._pattern);
    if (tag != null) {
      const o = this._options ?? {};
      write({
        t: "ac",
        source,
        g: tag.g,
        rule: tag.rule,
        input: text,
        options: {
          greedyPatternNames: o.greedyPatternNames ?? [],
          customTokens: o.customTokens ?? {},
          disableDedupe: Boolean(o.disableDedupe),
        },
        origin: origin(),
      });
    }
    return suggestFor.call(this, text);
  };
}
