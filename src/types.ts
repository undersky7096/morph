// Miroir des structures Rust exposées via IPC.

export type Kind = "image" | "audio" | "video";

/** Format cible indisponible sur cette machine (moteur absent), affiché grisé. */
export interface Blocked {
  format: string;
  reason: string;
}

export interface TargetGroup {
  kind: Kind;
  targets: string[];
  blocked: Blocked[];
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
