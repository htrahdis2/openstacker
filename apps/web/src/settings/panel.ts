/**
 * The settings screen.
 *
 * Every control is generated from the schema. There is no per-setting code here: a
 * setting added to the engine's descriptor tables appears with its bounds, its step, its
 * unit and its help text already correct.
 */

import { controlFor, duplicateBindings, readout, snapToStep } from "./controls";
import { SCHEMA, type Field, type Group, sections } from "./schema";
import type { Settings } from "./store";

export interface PanelOptions {
  settings: Settings;
  centiframes: (ms: number) => number;
  /** Called with the whole settings object whenever a control changes it. */
  onChange: (settings: Settings) => void;
  /** Groups that reach the simulation are frozen while a game is running. */
  locked: () => boolean;
}

/** Which stored section a schema group edits. */
const SECTION_OF: Record<string, keyof Settings | null> = {
  handling: "handling",
  keybinds: "keybinds",
  cosmetic: "cosmetic",
  // Match rules and the attack table come from the mode being played, not the player.
  match: null,
  attack_table: null,
};

export function buildPanel(root: HTMLElement, options: PanelOptions): void {
  root.innerHTML = "";
  for (const group of SCHEMA.groups) {
    root.append(groupElement(group, options));
  }
  refreshWarnings(root, options.settings);
}

function groupElement(group: Group, options: PanelOptions): HTMLElement {
  const section = document.createElement("section");
  section.className = "settings-group";

  const heading = document.createElement("h3");
  heading.textContent = group.label;
  section.append(heading);

  const owned = SECTION_OF[group.id] ?? null;
  if (owned === null) {
    const note = document.createElement("p");
    note.className = "settings-note";
    note.textContent = "fixed by this mode";
    section.append(note);
  }

  for (const block of sections(group)) {
    if (block.id !== group.id) {
      const sub = document.createElement("h4");
      sub.textContent = block.label;
      section.append(sub);
    }
    for (const field of block.fields) {
      section.append(fieldElement(group, field, owned, options));
    }
  }
  return section;
}

function fieldElement(
  group: Group,
  field: Field,
  owned: keyof Settings | null,
  options: PanelOptions,
): HTMLElement {
  const row = document.createElement("div");
  row.className = "settings-row";
  row.dataset.key = field.key;

  const label = document.createElement("label");
  label.textContent = field.label;
  label.title = field.help;
  row.append(label);

  const values = owned ? (options.settings[owned] as Record<string, unknown>) : null;
  const value = values?.[field.key] ?? ("default" in field ? field.default : "");

  const set = (next: unknown): void => {
    if (!owned || !values) return;
    values[field.key] = next as never;
    options.onChange(options.settings);
    refreshWarnings(row.closest(".settings-panel") ?? row, options.settings);
  };

  const control = buildControl(field, value, set, options);
  if (!owned || (group.affectsSimulation && options.locked())) {
    for (const input of control.querySelectorAll("input, select, button")) {
      (input as HTMLInputElement).disabled = true;
    }
    row.classList.add("locked");
  }
  row.append(control);

  if (field.help) {
    const help = document.createElement("p");
    help.className = "settings-help";
    help.textContent = field.help;
    row.append(help);
  }
  return row;
}

function buildControl(
  field: Field,
  value: unknown,
  set: (next: unknown) => void,
  options: PanelOptions,
): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "settings-control";

  switch (controlFor(field)) {
    case "slider": {
      if (field.type !== "int") break;
      const slider = document.createElement("input");
      slider.type = "range";
      slider.min = String(field.min);
      slider.max = String(field.max);
      slider.step = String(field.step);
      slider.value = String(value);

      const out = document.createElement("span");
      out.className = "settings-readout";
      out.textContent = readout(field, value, options.centiframes);

      slider.addEventListener("input", () => {
        const next = snapToStep(field, Number(slider.value));
        out.textContent = readout(field, next, options.centiframes);
        set(next);
      });
      wrap.append(slider, out);
      break;
    }

    case "toggle": {
      const toggle = document.createElement("input");
      toggle.type = "checkbox";
      toggle.checked = Boolean(value);
      toggle.addEventListener("change", () => set(toggle.checked));
      wrap.append(toggle);
      break;
    }

    case "choice": {
      if (field.type !== "enum") break;
      const select = document.createElement("select");
      for (const variant of field.variants) {
        const option = document.createElement("option");
        option.value = variant.value;
        option.textContent = variant.label;
        option.title = variant.help;
        select.append(option);
      }
      select.value = String(value);
      select.addEventListener("change", () => set(select.value));
      wrap.append(select);
      break;
    }

    case "capture": {
      // A key name is whatever the platform calls it, so the control captures one rather
      // than offering a list.
      const button = document.createElement("button");
      button.type = "button";
      button.className = "settings-capture";
      button.textContent = String(value) || "unbound";
      button.addEventListener("click", () => {
        button.textContent = "press a key";
        button.classList.add("capturing");
        const capture = (e: KeyboardEvent): void => {
          e.preventDefault();
          e.stopPropagation();
          window.removeEventListener("keydown", capture, true);
          button.classList.remove("capturing");
          if (e.code === "Escape") {
            button.textContent = String(value) || "unbound";
            return;
          }
          button.textContent = e.code;
          set(e.code);
        };
        window.addEventListener("keydown", capture, true);
      });
      wrap.append(button);
      break;
    }
  }
  return wrap;
}

/** Mark any action sharing a key with another. */
function refreshWarnings(root: Element, settings: Settings): void {
  for (const row of root.querySelectorAll(".settings-row")) {
    row.classList.remove("clash");
  }
  for (const clash of duplicateBindings(settings.keybinds)) {
    for (const action of clash) {
      root.querySelector(`.settings-row[data-key="${action}"]`)?.classList.add("clash");
    }
  }
}
