// Miroir des structures Rust exposées via IPC.

export type Kind = "image" | "audio" | "video" | "document" | "archive";

/** Format cible indisponible sur cette machine (moteur absent), affiché grisé. */
export interface Blocked {
  format: string;
  reason: string;
  /** Page pour obtenir le moteur qui manque (LibreOffice), ouverte au clic. */
  link: string | null;
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
  /** Dossiers déposés : ils s'archivent (ZIP, 7Z). */
  folders: string[];
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
