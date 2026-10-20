// Miroir des structures Rust exposées via IPC.

export type Kind = "image" | "audio" | "video";

export interface TargetGroup {
  kind: Kind;
  targets: string[];
  paths: string[];
}

export interface TargetMenu {
  common: string[];
  groups: TargetGroup[];
  unsupported: string[];
}

export interface RadialPayload {
  id: number;
  paths: string[];
  menu: TargetMenu;
  /** Premier anneau : formats, outils après un enregistrement, ou outils (outil fermé sans enregistrer). */
  ring: "formats" | "saved" | "tools";
}

export interface ConvertProgress {
  index: number;
  input: string;
  progress: number;
}
