// v12's `patterns` template tag and `createPatternsTemplate`: a grammar's
// rules by camelCase name.

import { Grammar } from "./grammar.mjs";

const kebabRegex = /-([a-z])/g;

function kebabToCamelCase(str) {
  return str.replace(kebabRegex, (_, char) => char.toUpperCase());
}

function combine(strings, values) {
  return strings.reduce(
    (result, str, i) => result + str + (values[i] ? String(values[i]) : ""),
    ""
  );
}

function byCamelCase(built) {
  const result = {};
  Object.keys(built).forEach(k => {
    result[kebabToCamelCase(k)] = built[k];
  });
  return result;
}

export function patterns(strings, ...values) {
  return byCamelCase(Grammar.parseString(combine(strings, values)));
}

export function createPatternsTemplate(options) {
  return function patterns(strings, ...values) {
    return byCamelCase(Grammar.parseString(combine(strings, values), options));
  };
}
