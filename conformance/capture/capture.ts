// Logs every (grammar, rule, input) a test suite runs through v12, by
// patching Grammar.prototype.parseString and AutoComplete. Only plain builds
// (no params, decorators or custom resolvers) are tagged: the corpus must be
// rebuildable from text alone.
import { createHash } from "node:crypto";
import { appendFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

type Tag = { g: string; rule: string };

export function install(Grammar: any, AutoComplete: any, source: string, label: (text: string) => string | null, origin: () => string) {
  const outDir = process.env.CAPTURE_OUT;
  if (!outDir) throw new Error("CAPTURE_OUT not set");
  mkdirSync(outDir, { recursive: true });
  const file = join(outDir, `${source}-${process.pid}-${Math.random().toString(36).slice(2)}.jsonl`);
  const write = (rec: object) => appendFileSync(file, JSON.stringify(rec) + "\n");
  const tags = new WeakMap<object, Tag>();
  const seenGrammars = new Set<string>();

  const isPlain = (g: any) =>
    (g._params?.length ?? 0) === 0 &&
    Object.keys(g._parseContext?.decorators ?? {}).every((k: string) => k === "tokens") &&
    g._sharedAnonymousNames == null &&
    g._resolveImportSync?.name === "defaultImportResolverSync";

  const original = Grammar.prototype.parseString;
  Grammar.prototype.parseString = function (expression: string) {
    if (!isPlain(this)) return original.call(this, expression);
    const g = createHash("sha1").update(expression).digest("hex").slice(0, 16);
    const fileLabel = label(expression);
    const logGrammar = (error: string | null) => {
      if (seenGrammars.has(g)) return;
      seenGrammars.add(g);
      write({ t: "grammar", source, g, file: fileLabel, text: expression, error, origin: origin() });
    };
    let patterns: Record<string, any>;
    try {
      patterns = original.call(this, expression);
    } catch (e) {
      logGrammar((e as Error).message);
      throw e;
    }
    logGrammar(null);
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
    return patterns;
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
