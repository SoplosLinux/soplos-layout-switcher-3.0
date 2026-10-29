export interface EnvironmentInfo {
  desktopEnvironment: string;
  displayProtocol: string;
  shellVersion: string;
}

export interface Switches {
  tray: boolean;
  desktopIcons: boolean;
}

export type LayoutId = "soplos" | "dock" | "tiling" | "gnome";

// The four layouts in the places they have in the 2.x window: Dock and Soplos
// on the top row, Tiling and GNOME below.
export const LAYOUTS: LayoutId[] = ["dock", "soplos", "tiling", "gnome"];

export const DEFAULT_LAYOUT: LayoutId = "soplos";

export const isBuiltIn = (id: string | null | undefined): id is LayoutId => !!id && (LAYOUTS as string[]).includes(id);

export type Side = "top" | "bottom" | "left" | "right";

/** The shape of a saved layout, read from its extensions and their settings. */
export interface LayoutPreviewShape {
  /** Dash to Panel, in the place of the top bar. */
  panel: Side | null;
  dock: Side | null;
  dockFull: boolean;
  tiling: boolean;
  menu: boolean;
  /** Open Bar: the top bar as islands. */
  islands: boolean;
  desktopIcons: boolean;
}

/** A layout the user saved: the program names it, the user chose its name. */
export interface CustomLayout {
  id: string;
  name: string;
  preview: LayoutPreviewShape;
}

// What a name of a layout can hold, as the program checks it: short and plain.
export const MAX_NAME_CHARS = 40;
export const validLayoutName = (name: string) => /^[\p{L}\p{N} .,_-]+$/u.test(name.trim()) && name.trim().length > 0 && name.trim().length <= MAX_NAME_CHARS && !name.trim().startsWith("-");

// The tools of the Settings page, in the order of the 2.x window, with the
// alternative to GNOME Tweaks beside it.
export type ToolId = "tweaks" | "refine" | "extensions" | "accent" | "gdm" | "firefox";
export const TOOLS: ToolId[] = ["tweaks", "refine", "extensions", "accent", "gdm", "firefox"];
