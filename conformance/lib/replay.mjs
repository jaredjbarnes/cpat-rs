// Rebuilds a captured grammar in a clarity-pattern-parser-shaped API from
// its text and the record of its options (conformance/capture/capture.ts):
// params by value, imports answered from the recorded resolver calls, and
// each custom decorator replaced by one that records what it received. The
// native engine (conformance/native/src/replay.rs) replays the same record.

/** What a decorator received, as the build result reports it:
 * `[decorator, pattern name, pattern type, JSON.stringify(arg) or null]`. */
export const decoration = (decorator, pattern, arg) => [
  decorator,
  pattern.name,
  pattern.type,
  arg === undefined ? null : JSON.stringify(arg),
];

/** The recorded answer to `resource` asked from `origin`. An import no
 * recorded call answers is an error the engines all report the same. */
function answer(calls, resource, origin) {
  const call = calls.find(c => c.resource === resource && c.origin === (origin ?? null));
  if (call == null) return { error: `unrecorded import ${JSON.stringify(resource)} from ${JSON.stringify(origin ?? null)}` };
  return call;
}

/** GrammarOptions for `api` from a recorded options object, and the list
 * the recording decorators fill. */
export function replayOptions(api, options = {}) {
  const decorated = [];
  const grammarOptions = {};
  if (options.originResource != null) grammarOptions.originResource = options.originResource;
  if (options.params != null) {
    grammarOptions.params = options.params.map(p =>
      p.type === "literal" ? new api.Literal(p.name, p.value) : new api.Regex(p.name, p.regex)
    );
  }
  if (options.decorators != null) {
    grammarOptions.decorators = Object.fromEntries(
      options.decorators.map(name => [
        name,
        (pattern, arg) => {
          decorated.push(decoration(name, pattern, arg));
        },
      ])
    );
  }
  if (options.resolveImportSync != null) {
    grammarOptions.resolveImportSync = (resource, origin) => {
      const call = answer(options.resolveImportSync, resource, origin);
      if (call.error != null) throw new Error(call.error);
      return { ...call.file };
    };
  }
  if (options.resolveImport != null) {
    grammarOptions.resolveImport = async (resource, origin) => {
      const call = answer(options.resolveImport, resource, origin);
      if (call.error != null) throw new Error(call.error);
      return { ...call.file };
    };
  }
  return { grammarOptions, decorated };
}

/** Builds `grammar` as it was captured: `{ patterns, error, decorated }`,
 * with `error` what the build threw (`threw` normalizes it). */
export async function replayBuild(api, grammar, threw) {
  const options = grammar.options ?? { entry: "parseString" };
  const { grammarOptions, decorated } = replayOptions(api, options);
  try {
    const patterns =
      options.entry === "parse"
        ? await api.Grammar.parse(grammar.text, grammarOptions)
        : api.Grammar.parseString(grammar.text, grammarOptions);
    return { patterns, error: null, decorated };
  } catch (error) {
    return { patterns: null, error: threw(error), decorated };
  }
}
