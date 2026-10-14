// Compares each engine's normalized results with TS v12's, field by field.
// A difference names its kind (which observable disagreed), the case, and
// both values, so the allowlist can explain it precisely.

/** The fields of a build after its error: the rule names, and what each
 * custom decorator received. */
const BUILD_FIELDS = ["rules", "decorated"];
/** The fields of an exec result, each its own kind of difference. */
const EXEC_FIELDS = ["matched", "ast", "error", "furthest", "message", "tooDeep"];
/** AutoComplete's, likewise. */
const AUTOCOMPLETE_FIELDS = ["complete", "at", "options", "tooDeep"];

const same = (a, b) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

/** Results by case id, and builds by grammar key, from JSON lines. */
export function indexResults(lines) {
  const cases = new Map();
  const builds = new Map();
  for (const line of lines) {
    if (line.build != null) builds.set(line.build, line);
    else cases.set(line.id, line);
  }
  return { cases, builds };
}

/** Counts of what was compared, and every difference. */
export function compare(corpus, expected, actual, engine) {
  const counts = { cases: 0, builds: 0, matches: 0, asts: 0, errors: 0, messages: 0, suggestions: 0 };
  const differences = [];

  for (const file of corpus.files) {
    for (const grammar of file.grammars) {
      const where = { engine, file: file.name, grammar: grammar.key };
      const want = expected.builds.get(grammar.key);
      const got = actual.builds.get(grammar.key);
      counts.builds++;
      // A different error is one difference; the fields after it mean nothing.
      if (!same(want?.error, got?.error)) {
        differences.push({ ...where, kind: "build.error", expected: want?.error, actual: got?.error });
      } else {
        for (const field of BUILD_FIELDS) {
          if (!same(want?.[field], got?.[field])) {
            differences.push({ ...where, kind: `build.${field}`, expected: want?.[field] ?? null, actual: got?.[field] ?? null });
          }
        }
      }

      for (const c of grammar.cases) {
        counts.cases++;
        const at = { ...where, id: c.id, rule: c.rule, input: c.input, caseKind: c.kind };
        const want = expected.cases.get(c.id) ?? { lost: true };
        const got = actual.cases.get(c.id) ?? { lost: true };
        compareCase(want, got, at, counts, differences);
      }
    }
  }
  return { counts, differences };
}

function compareCase(want, got, at, counts, differences) {
  for (const key of ["lost", "missing"]) {
    if (!same(want[key], got[key])) {
      differences.push({ ...at, kind: `case.${key}`, expected: want[key], actual: got[key] });
      return;
    }
  }
  if (want.exec != null || got.exec != null) {
    compareFields("exec", want.exec ?? {}, got.exec ?? {}, EXEC_FIELDS, at, differences);
    counts.matches++;
    if (want.exec?.ast != null) counts.asts++;
    if (want.exec?.error != null || want.exec?.furthest != null) counts.errors++;
    counts.messages++;
  }
  if (want.autocomplete != null || got.autocomplete != null) {
    compareFields("autocomplete", want.autocomplete ?? {}, got.autocomplete ?? {}, AUTOCOMPLETE_FIELDS, at, differences);
    counts.suggestions++;
  }
}

function compareFields(section, want, got, fields, at, differences) {
  // A throw on either side is one difference; the fields after it mean nothing.
  if (!same(want.threw, got.threw)) {
    differences.push({ ...at, kind: `${section}.threw`, expected: want.threw ?? null, actual: got.threw ?? null, want, got });
    return;
  }
  for (const field of fields) {
    if (!same(want[field], got[field])) {
      differences.push({ ...at, kind: `${section}.${field}`, expected: want[field] ?? null, actual: got[field] ?? null, want, got });
    }
  }
}
