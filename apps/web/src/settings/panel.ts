/**
 * The settings screen.
 *
 * Every control is generated from the schema. There is no per-setting code here: a
 * setting added to the engine's descriptor tables appears with its bounds, its step, its
 * unit and its help text already correct.
 */

import { clampToList, controlFor, duplicateBindings, readout, snapToStep } from "./controls";
import { SCHEMA, type Field, type Group, sections } from "./schema";
import type { Rules, Settings } from "./store";

export interface PanelOptions {
  settings: Settings;
  centiframes: (ms: number) => number;
  /** Called with the whole settings object whenever a control changes it. */
  onChange: (settings: Settings) => void;
  /** Groups that reach the simulation are frozen while a game is running. */
  locked: () => boolean;
  /** Rules of the mode being tuned, which house rules start from and are compared to. */
  modeRules: () => Rules | null;
  /** Its name, so the player can see which mode they are tuning against. */
  modeName: () => string | null;
  /** Start tuning, seeded from the mode's rules. */
  onTune: () => void;
  /** Go back to playing the mode as written. */
  onUntune: () => void;
  /** The tuned rules as a mode file's `[config]` block. */
  toToml: () => string;
}

/**
 * The object a schema group edits, or null when there is nothing to edit into.
 *
 * Match rules and the attack table edit the player's house rules, which exist only once
 * they have chosen to tune something. Until then they are the mode's, and read-only.
 */
function editable(group: string, settings: Settings): Record<string, unknown> | null {
  switch (group) {
    case "handling":
      return settings.handling;
    case "keybinds":
      return settings.keybinds;
    case "cosmetic":
      return settings.cosmetic;
    case "match":
      return (settings.house_rules as Record<string, unknown>) ?? null;
    case "attack_table":
      return (settings.house_rules?.attack_table as Record<string, unknown>) ?? null;
    default:
      return null;
  }
}

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

  const values = editable(group.id, options.settings);
  const rules = group.id === "match" || group.id === "attack_table";
  if (rules) {
    section.append(rulesHeader(group, options));
  } else if (values === null) {
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
      section.append(fieldElement(group, field, values, options));
    }
  }
  return section;
}

/**
 * The line above the match rules: whose they are, and how to make them yours.
 *
 * A control a player can drag that silently does nothing is worse than a disabled one,
 * so until they tune, these say who fixed them and stay locked.
 */
function rulesHeader(group: Group, options: PanelOptions): HTMLElement {
  const row = document.createElement("div");
  row.className = "settings-rules";
  const tuned = options.settings.house_rules !== undefined;

  const mode = options.modeName();
  const note = document.createElement("p");
  note.className = "settings-note";
  note.textContent = tuned
    ? `your rules${mode ? `, from ${mode}` : ""}`
    : `fixed by ${mode ?? "this mode"}`;
  row.append(note);

  if (group.id !== "match") return row;

  if (!tuned) {
    const tune = document.createElement("button");
    tune.type = "button";
    tune.textContent = "tune these";
    tune.addEventListener("click", options.onTune);
    row.append(tune);
    return row;
  }

  const revert = document.createElement("button");
  revert.type = "button";
  revert.textContent = "back to the mode";
  revert.addEventListener("click", options.onUntune);

  // Tuning that cannot leave the browser is tuning that gets lost.
  const copy = document.createElement("button");
  copy.type = "button";
  copy.textContent = "copy as TOML";
  copy.addEventListener("click", () => {
    const text = options.toToml();
    void navigator.clipboard?.writeText(text);
    copy.textContent = text ? "copied" : "nothing changed";
    setTimeout(() => (copy.textContent = "copy as TOML"), 1200);
  });

  row.append(revert, copy);
  return row;
}

function fieldElement(
  group: Group,
  field: Field,
  values: Record<string, unknown> | null,
  options: PanelOptions,
): HTMLElement {
  const row = document.createElement("div");
  row.className = "settings-row";
  row.dataset.key = field.key;

  const label = document.createElement("label");
  label.textContent = field.label;
  label.title = field.help;
  row.append(label);

  const fallback = fromMode(group, field, options) ?? ("default" in field ? field.default : "");
  const value = values?.[field.key] ?? fallback;

  const set = (next: unknown): void => {
    if (!values) return;
    values[field.key] = next as never;
    options.onChange(options.settings);
    refreshWarnings(row.closest(".settings-panel") ?? row, options.settings);
  };

  const control = buildControl(field, value, set, options);
  if (!values || (group.affectsSimulation && options.locked())) {
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

/** What the mode being played sets a rule to, so a locked control shows the real value. */
function fromMode(group: Group, field: Field, options: PanelOptions): unknown {
  const rules = options.modeRules();
  if (!rules) return undefined;
  if (group.id === "attack_table") {
    return (rules.attack_table as Record<string, unknown> | undefined)?.[field.key];
  }
  return group.id === "match" ? rules[field.key] : undefined;
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

    case "table": {
      if (field.type !== "intList") break;
      // One entry per position, labelled by the position, because that is what the
      // number means: the reward for a run of that length.
      const current = Array.isArray(value) ? (value as number[]).slice() : [...field.default];
      const list = document.createElement("div");
      list.className = "settings-table";

      const rebuild = (): void => {
        list.innerHTML = "";
        current.forEach((entry, i) => {
          const cell = document.createElement("label");
          cell.className = "settings-table-cell";
          const index = document.createElement("span");
          index.textContent = String(i);
          const input = document.createElement("input");
          input.type = "number";
          input.min = String(field.min);
          input.max = String(field.max);
          input.value = String(entry);
          input.addEventListener("change", () => {
            current[i] = Number(input.value);
            set(clampToList(field, current));
          });
          cell.append(index, input);
          list.append(cell);
        });
      };
      rebuild();

      const shorter = document.createElement("button");
      shorter.type = "button";
      shorter.textContent = "−";
      shorter.title = "one entry fewer";
      shorter.addEventListener("click", () => {
        if (current.length <= 1) return;
        current.pop();
        set(clampToList(field, current));
        rebuild();
      });

      const longer = document.createElement("button");
      longer.type = "button";
      longer.textContent = "+";
      longer.title = "one entry more";
      longer.addEventListener("click", () => {
        if (current.length >= field.maxLen) return;
        current.push(current[current.length - 1] ?? 0);
        set(clampToList(field, current));
        rebuild();
      });

      const length = document.createElement("div");
      length.className = "settings-table-length";
      length.append(shorter, longer);
      wrap.append(list, length);
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
