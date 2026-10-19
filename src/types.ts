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
}

export interface ConvertProgress {
  index: number;
  input: string;
  progress: number;
}

export interface ConvertResult {
  input: string;
  output: string | null;
  error: string | null;
}
