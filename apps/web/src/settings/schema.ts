/**
 * The settings schema, as the engine describes itself.
 *
 * Every control on the settings screen is built from this. Nothing about an individual
 * setting is written in TypeScript, so adding one is a change in Rust and the control
 * appears. CI fails if the committed schema falls behind the descriptor tables.
 */

import schema from "../../../../config-schema.json";

export interface IntField {
  key: string;
  label: string;
  help: string;
  group: string;
  type: "int";
  min: number;
  max: number;
  default: number;
  step: number;
  unit: string;
}

export interface BoolField {
  key: string;
  label: string;
  help: string;
  group: string;
  type: "bool";
  default: boolean;
}

export interface EnumVariant {
  value: string;
  label: string;
  help: string;
}

export interface EnumField {
  key: string;
  label: string;
  help: string;
  group: string;
  type: "enum";
  default: string;
  variants: EnumVariant[];
}

export interface BindingField {
  key: string;
  label: string;
  help: string;
  group: string;
  type: "binding";
  default: string;
  variants: EnumVariant[];
}

export interface IntListField {
  key: string;
  label: string;
  help: string;
  group: string;
  type: "intList";
  min: number;
  max: number;
  maxLen: number;
  default: number[];
  unit: string;
}

export type Field = IntField | BoolField | EnumField | BindingField | IntListField;

export interface Group {
  id: string;
  label: string;
  affectsSimulation: boolean;
  fields: Field[];
  nested: string[];
}

export interface Schema {
  version: number;
  groups: Group[];
}

export const SCHEMA: Schema = schema as Schema;

export function group(id: string): Group | undefined {
  return SCHEMA.groups.find((g) => g.id === id);
}

/** Fields of a group, split by the UI section each declares. */
export function sections(g: Group): { id: string; label: string; fields: Field[] }[] {
  const order: string[] = [];
  const byGroup = new Map<string, Field[]>();
  for (const field of g.fields) {
    if (!byGroup.has(field.group)) {
      byGroup.set(field.group, []);
      order.push(field.group);
    }
    byGroup.get(field.group)!.push(field);
  }
  return order.map((id) => ({ id, label: sectionLabel(id), fields: byGroup.get(id)! }));
}

/** The trailing part of a dotted group name, spelled for a heading. */
export function sectionLabel(id: string): string {
  const last = id.split(".").pop() ?? id;
  return last.replace(/_/g, " ");
}

/** Whether a duration field should show a frame read-out beside it. */
export function showsFrames(field: Field): boolean {
  return field.type === "int" && field.unit === "ms";
}
