// The allowlist: every difference from TS v12 that is explained. An entry
// matches differences by kind (or kinds) and, optionally, by engine, corpus
// file, rule, input (exact or by regex), the two values and a named check
// (allowlist_checks.mjs); each must cite the decision behind it.
// An entry that explains nothing is stale and fails the run too, so the
// list can only describe differences that really occur.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { CHECKS } from "./allowlist_checks.mjs";
import { conformanceDir } from "./paths.mjs";

export const allowlistPath = join(conformanceDir, "allowlist.json");

const REQUIRED = ["id", "reason", "cites"];

export function loadAllowlist(path = allowlistPath) {
  const entries = JSON.parse(readFileSync(path, "utf8"));
  for (const entry of entries) {
    for (const key of REQUIRED) {
      if (typeof entry[key] !== "string" || entry[key].length === 0) {
        throw new Error(`allowlist entry ${JSON.stringify(entry.id)} needs a non-empty \`${key}\``);
      }
    }
    const kinds = entry.kinds ?? [entry.kind];
    if (kinds.length === 0 || kinds.some(kind => typeof kind !== "string" || kind.length === 0)) {
      throw new Error(`allowlist entry ${entry.id} needs a \`kind\` or a non-empty \`kinds\``);
    }
    if (entry.check !== undefined && CHECKS[entry.check] == null) {
      throw new Error(`allowlist entry ${entry.id} names an unknown check ${JSON.stringify(entry.check)}`);
    }
    if (!/DESIGN\.md|MIGRATION-v12\.md/.test(entry.cites)) {
      throw new Error(`allowlist entry ${entry.id} must cite DESIGN.md or MIGRATION-v12.md`);
    }
  }
  return entries;
}

const valueMatches = (value, exact, pattern) =>
  (exact === undefined || JSON.stringify(exact) === JSON.stringify(value ?? null)) &&
  (pattern === undefined || new RegExp(pattern).test(String(value)));

export function explains(entry, difference) {
  return (
    (entry.kinds ?? [entry.kind]).includes(difference.kind) &&
    (entry.engines === undefined || entry.engines.includes(difference.engine)) &&
    (entry.file === undefined || entry.file === difference.file) &&
    (entry.rule === undefined || entry.rule === difference.rule) &&
    (entry.input === undefined || entry.input === difference.input) &&
    (entry.inputPattern === undefined || new RegExp(entry.inputPattern, "u").test(difference.input ?? "")) &&
    (entry.check === undefined || CHECKS[entry.check](difference)) &&
    valueMatches(difference.expected, entry.expected, entry.expectedPattern) &&
    valueMatches(difference.actual, entry.actual, entry.actualPattern) &&
    // Fields the cpat engine's whole result must have, beyond the one that
    // differs (a halt's `tooDeep`, for instance).
    Object.entries(entry.got ?? {}).every(([key, value]) => JSON.stringify(difference.got?.[key]) === JSON.stringify(value))
  );
}

/** Splits differences into explained (with the entry) and unexplained,
 * and counts what each entry explained. */
export function applyAllowlist(entries, differences) {
  const explained = [];
  const unexplained = [];
  const uses = new Map(entries.map(entry => [entry.id, 0]));
  for (const difference of differences) {
    const entry = entries.find(e => explains(e, difference));
    if (entry == null) {
      unexplained.push(difference);
    } else {
      explained.push({ ...difference, allowlist: entry.id });
      uses.set(entry.id, uses.get(entry.id) + 1);
    }
  }
  const stale = entries.filter(entry => uses.get(entry.id) === 0);
  return { explained, unexplained, uses, stale };
}
