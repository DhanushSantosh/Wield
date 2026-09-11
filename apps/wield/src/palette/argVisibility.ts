import type { ArgSpec, ArgValueLiteral } from "../lib/wield";

export function literalValue(literal: ArgValueLiteral): string | number | boolean {
  if ("Str" in literal) return literal.Str;
  if ("Int" in literal) return literal.Int;
  if ("Float" in literal) return literal.Float;
  return literal.Bool;
}

function isVisibleInner(
  spec: ArgSpec,
  values: Record<string, unknown>,
  allSpecs: ArgSpec[],
  visited: Set<string>,
): boolean {
  if (spec.when === null) return true;
  if (visited.has(spec.name)) return false;
  const nextVisited = new Set(visited).add(spec.name);
  const referenced = allSpecs.find((candidate) => candidate.name === spec.when?.arg);
  if (referenced && !isVisibleInner(referenced, values, allSpecs, nextVisited)) return false;
  const isSet = Object.hasOwn(values, spec.when.arg) && values[spec.when.arg] !== undefined;
  if (spec.when.in.length === 0) return isSet;
  return isSet && spec.when.in.some((literal) => literalValue(literal) === values[spec.when!.arg]);
}

export function isVisible(
  spec: ArgSpec,
  values: Record<string, unknown>,
  allSpecs: ArgSpec[],
): boolean {
  return isVisibleInner(spec, values, allSpecs, new Set());
}
