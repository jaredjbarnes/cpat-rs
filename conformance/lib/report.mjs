// The run's report: what was compared, per corpus file and in total, and
// the differences grouped by kind, explained ones with their allowlist
// entry. Printed, and written as markdown next to the results.

const show = value => {
  const text = JSON.stringify(value ?? null);
  return text.length > 240 ? `${text.slice(0, 240)}…` : text;
};

/** The first row where two AST row lists part, for an `exec.ast` difference. */
function astPart(expected, actual) {
  const a = (expected ?? "").split("\n");
  const b = (actual ?? "").split("\n");
  const at = a.findIndex((row, i) => row !== b[i]);
  const i = at < 0 ? a.length : at;
  return `first differing node row ${i}: ts ${show(a[i])} vs ${show(b[i])}`;
}

function describe(d) {
  const where = d.id == null ? `build of ${d.grammar}` : `#${d.id} ${d.caseKind} ${d.rule} ${show(d.input)}`;
  const values = d.kind === "exec.ast" ? astPart(d.expected, d.actual) : `ts ${show(d.expected)} vs ${show(d.actual)}`;
  return `${d.engine} ${d.file} ${where}\n      ${values}`;
}

function groupByKind(differences) {
  const groups = new Map();
  for (const d of differences) {
    const key = d.allowlist == null ? d.kind : `${d.kind} (${d.allowlist})`;
    groups.set(key, [...(groups.get(key) ?? []), d]);
  }
  return [...groups].sort((a, b) => b[1].length - a[1].length);
}

export function formatReport({ perFile, totals, engines, explained, unexplained, stale, examples }) {
  const lines = [];
  lines.push("# Conformance: TS v12 vs cpat-rs native vs cpat-wasm", "");
  lines.push("| corpus | suite | grammars | captured | cases | autocomplete |");
  lines.push("|---|---|---:|---:|---:|---:|");
  for (const f of perFile) {
    lines.push(`| ${f.name} | ${f.suite} | ${f.grammars} | ${f.captured} | ${f.cases} | ${f.suggestions} |`);
  }
  lines.push("");
  lines.push(`Engines compared with TS v12: ${engines.join(", ")}.`, "");
  lines.push("| compared (per engine) | count |", "|---|---:|");
  lines.push(`| cases | ${totals.cases} |`);
  lines.push(`| grammar builds | ${totals.builds} |`);
  lines.push(`| matches (match / no match) | ${totals.matches} |`);
  lines.push(`| ASTs (cases where v12 matched) | ${totals.asts} |`);
  lines.push(`| errors (cases with an error or furthest error) | ${totals.errors} |`);
  lines.push(`| error messages | ${totals.messages} |`);
  lines.push(`| suggestions (AutoComplete) | ${totals.suggestions} |`);
  lines.push(`| differences explained by the allowlist | ${explained.length} |`);
  lines.push(`| differences unexplained | ${unexplained.length} |`);
  lines.push("");

  for (const [title, list] of [
    ["Unexplained differences", unexplained],
    ["Explained differences", explained],
  ]) {
    lines.push(`## ${title} (${list.length})`, "");
    for (const [kind, group] of groupByKind(list)) {
      lines.push(`### ${kind}: ${group.length}`, "");
      lines.push("```");
      for (const d of group.slice(0, examples)) lines.push(`  ${describe(d)}`);
      if (group.length > examples) lines.push(`  … and ${group.length - examples} more`);
      lines.push("```", "");
    }
  }
  if (stale.length > 0) {
    lines.push(`## Stale allowlist entries (${stale.length})`, "");
    for (const entry of stale) lines.push(`- \`${entry.id}\` explains no difference; remove it.`);
    lines.push("");
  }
  return lines.join("\n");
}

export function summaryLine({ totals, explained, unexplained, stale }) {
  return (
    `cases ${totals.cases}, matches compared ${totals.matches}, ASTs compared ${totals.asts}, ` +
    `errors compared ${totals.errors}, messages compared ${totals.messages}, suggestions compared ${totals.suggestions}; ` +
    `differences ${explained.length + unexplained.length} (explained ${explained.length}, unexplained ${unexplained.length})` +
    (stale.length > 0 ? `; stale allowlist entries ${stale.length}` : "")
  );
}
