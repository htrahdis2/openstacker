/**
 * Skins.
 *
 * A skin maps a render-channel index to a color: 0 empty, 1..7 the piece kinds in engine
 * order (I O T S Z J L), 8 garbage. Nothing else reads these indices — the simulation
 * never sees a color, and the renderer never asks a color what a cell is.
 *
 * The mapping is deliberately not the one every falling-block game uses. Piece coloring is
 * the most identifiable protected element of the genre's expression, so this palette pairs
 * shapes with hues that are ours.
 */

export interface Skin {
  id: string;
  label: string;
  /** Indexed by render-channel value, 0..=8. */
  cells: string[];
  grid: string;
  background: string;
  text: string;
  dim: string;
}

const DEFAULT: Skin = {
  id: "default",
  label: "Default",
  cells: [
    "transparent",
    "#f0568c", // I
    "#3fc6b4", // O
    "#f0a93f", // T
    "#6d78e0", // S
    "#a8cf4a", // Z
    "#e0603f", // J
    "#9b6fd6", // L
    "#575d6b", // garbage
  ],
  grid: "#22252d",
  background: "#14161b",
  text: "#e8eaef",
  dim: "#8b909c",
};

const MONO: Skin = {
  id: "mono",
  label: "Monochrome",
  cells: [
    "transparent",
    "#c8ccd6",
    "#c8ccd6",
    "#c8ccd6",
    "#c8ccd6",
    "#c8ccd6",
    "#c8ccd6",
    "#c8ccd6",
    "#5b606b",
  ],
  grid: "#212429",
  background: "#131519",
  text: "#e8eaef",
  dim: "#8b909c",
};

const HIGH_CONTRAST: Skin = {
  id: "high_contrast",
  label: "High contrast",
  cells: [
    "transparent",
    "#ff2d6f",
    "#00e5c0",
    "#ffb300",
    "#3d5bff",
    "#7cff2d",
    "#ff5c1a",
    "#c04dff",
    "#8b8f99",
  ],
  grid: "#000000",
  background: "#000000",
  text: "#ffffff",
  dim: "#b8bcc6",
};

const SKINS: Skin[] = [DEFAULT, MONO, HIGH_CONTRAST];

export function skin(id: string): Skin {
  return SKINS.find((s) => s.id === id) ?? DEFAULT;
}

export function skinIds(): string[] {
  return SKINS.map((s) => s.id);
}
