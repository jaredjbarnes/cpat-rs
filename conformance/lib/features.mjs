// Which of the grammar language's composition features a corpus grammar
// uses, for the report's counts: imports (a statement, or an import
// resolver the build was given), params (`use params`, `with params`, or
// params passed in) and decorators (any `@name` statement, built-in
// `@tokens` included, or custom decorators passed in).

const IMPORT = /^\s*import\s*\{/m;
const PARAMS = /^\s*use\s+params\s*\{|\bwith\s+params\s*\{/m;
const DECORATOR = /^\s*@[A-Za-z_][\w-]*/m;

export function grammarUses({ text, options = {} }) {
  return {
    imports: IMPORT.test(text) || options.resolveImport != null || options.resolveImportSync != null,
    params: PARAMS.test(text) || (options.params?.length ?? 0) > 0,
    decorators: DECORATOR.test(text) || (options.decorators?.length ?? 0) > 0,
  };
}
