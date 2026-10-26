import type { SVGProps } from "react";

const base: SVGProps<SVGSVGElement> = {
  width: 20,
  height: 20,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
};

export const CropIcon = () => (
  <svg {...base}>
    <path d="M6 2v14a2 2 0 0 0 2 2h14" />
    <path d="M18 22V8a2 2 0 0 0-2-2H2" />
  </svg>
);

export const CompressIcon = () => (
  <svg {...base}>
    <path d="M4 14h6v6M20 10h-6V4M14 10l7-7M3 21l7-7" />
  </svg>
);

export const EditIcon = () => (
  <svg {...base}>
    <path d="M12 20h9" />
    <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
  </svg>
);

/** Silhouette détourée dans un cadre en pointillés. */
export const CutoutIcon = () => (
  <svg {...base}>
    <path d="M3 7V3h4M10 3h4M17 3h4v4M21 10v4M21 17v4h-4M14 21h-4M7 21H3v-4M3 14v-4" />
    <circle cx="12" cy="9.5" r="3" />
    <path d="M6.5 18.5a5.5 5.5 0 0 1 11 0" />
  </svg>
);

export const MergeIcon = () => (
  <svg {...base}>
    <path d="M8 3H5a2 2 0 0 0-2 2v6M16 3h3a2 2 0 0 1 2 2v6M12 21V11M8 7l4 4 4-4M5 21h14" />
  </svg>
);

export const ExtractIcon = () => (
  <svg {...base}>
    <path d="M21 8v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8M1 3h22v5H1zM12 11v7M9 15l3 3 3-3" />
  </svg>
);

export const CloseIcon = () => (
  <svg {...base} width={16} height={16}>
    <path d="M6 6l12 12M18 6 6 18" />
  </svg>
);

export const MinimizeIcon = () => (
  <svg {...base} width={16} height={16}>
    <path d="M5 12h14" />
  </svg>
);

export const ChevronIcon = () => (
  <svg {...base} width={14} height={14}>
    <path d="m6 9 6 6 6-6" />
  </svg>
);

// Éditeur image : barre d'outils et outils d'annotation.
const small = { ...base, width: 18, height: 18 };

export const UndoIcon = () => (
  <svg {...small}>
    <path d="M9 14 4 9l5-5" />
    <path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11" />
  </svg>
);

export const RedoIcon = () => (
  <svg {...small}>
    <path d="m15 14 5-5-5-5" />
    <path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13" />
  </svg>
);

export const RotateLeftIcon = () => (
  <svg {...small}>
    <path d="M3 4v5h5" />
    <path d="M3.5 13a8.5 8.5 0 1 0 2-7.5L3 9" />
  </svg>
);

export const RotateRightIcon = () => (
  <svg {...small}>
    <path d="M21 4v5h-5" />
    <path d="M20.5 13a8.5 8.5 0 1 1-2-7.5L21 9" />
  </svg>
);

export const FlipHIcon = () => (
  <svg {...small}>
    <path d="M12 3v18" strokeDasharray="2 2" />
    <path d="M9 7 3 17h6ZM15 7l6 10h-6Z" />
  </svg>
);

export const FlipVIcon = () => (
  <svg {...small}>
    <path d="M3 12h18" strokeDasharray="2 2" />
    <path d="M7 9 17 3v6ZM7 15l10 6v-6Z" />
  </svg>
);

export const PointerIcon = () => (
  <svg {...small}>
    <path d="m5 3 14 7-6 2-2 6Z" />
  </svg>
);

export const TextIcon = () => (
  <svg {...small}>
    <path d="M5 6V4h14v2M12 4v16M9 20h6" />
  </svg>
);

export const BrushIcon = () => (
  <svg {...small}>
    <path d="M3 17c3 0 3 4 7 3 2-.5 2.5-2.5 1.5-4L20 7.5a2 2 0 0 0-3-3L8.5 13C7 12 5 12.5 4.5 14.5" />
  </svg>
);

export const HighlighterIcon = () => (
  <svg {...small}>
    <path d="m9 11-5 5v3h3l5-5" />
    <path d="m9 11 6-6 4 4-6 6Z" />
    <path d="M4 22h16" />
  </svg>
);

export const EraserIcon = () => (
  <svg {...small}>
    <path d="m7 21-4-4a2 2 0 0 1 0-3l10-10a2 2 0 0 1 3 0l5 5a2 2 0 0 1 0 3l-8 9" />
    <path d="M7 21h13M9 11l6 6" />
  </svg>
);

export const ArrowIcon = () => (
  <svg {...small}>
    <path d="M5 19 19 5M10 5h9v9" />
  </svg>
);

export const LineIcon = () => (
  <svg {...small}>
    <path d="M5 19 19 5" />
  </svg>
);

export const RectIcon = () => (
  <svg {...small}>
    <rect x="4" y="6" width="16" height="12" rx="1.5" />
  </svg>
);

export const EllipseIcon = () => (
  <svg {...small}>
    <ellipse cx="12" cy="12" rx="9" ry="6.5" />
  </svg>
);

export const TrashIcon = () => (
  <svg {...small}>
    <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
  </svg>
);

// Éditeur audio : lecture, découpe, zoom.
export const PlayIcon = () => (
  <svg {...small} fill="currentColor" stroke="none">
    <path d="M7 4.5v15a1 1 0 0 0 1.5.86l12-7.5a1 1 0 0 0 0-1.72l-12-7.5A1 1 0 0 0 7 4.5Z" />
  </svg>
);

export const PauseIcon = () => (
  <svg {...small} fill="currentColor" stroke="none">
    <rect x="6" y="4" width="4" height="16" rx="1" />
    <rect x="14" y="4" width="4" height="16" rx="1" />
  </svg>
);

export const ScissorsIcon = () => (
  <svg {...small}>
    <circle cx="6" cy="6" r="3" />
    <circle cx="6" cy="18" r="3" />
    <path d="M20 4 8.1 15.9M14.5 14.5 20 20M8.1 8.1 12 12" />
  </svg>
);

export const KeepIcon = () => (
  <svg {...small}>
    <path d="M4 4v16M20 4v16" />
    <rect x="8" y="8" width="8" height="8" rx="1.5" />
  </svg>
);

export const ZoomInIcon = () => (
  <svg {...small}>
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-4-4M11 8v6M8 11h6" />
  </svg>
);

export const ZoomOutIcon = () => (
  <svg {...small}>
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-4-4M8 11h6" />
  </svg>
);

export const FitIcon = () => (
  <svg {...small}>
    <path d="M3 12h18M7 8l-4 4 4 4M17 8l4 4-4 4" />
  </svg>
);

// Réglages : navigation et accueil (même trait que les outils, plus fin).
const nav = { ...base, width: 16, height: 16, strokeWidth: 1.7 };

export const HomeIcon = () => (
  <svg {...nav}>
    <path d="M3 10.5 12 3l9 7.5" />
    <path d="M5 9v11h5v-6h4v6h5V9" />
  </svg>
);

export const PaletteIcon = () => (
  <svg {...nav}>
    <path d="M12 3a9 9 0 1 0 0 18c1.1 0 1.6-.9 1.2-1.8l-.4-.8c-.5-1 .2-2.4 1.4-2.4H17a4 4 0 0 0 4-4c0-5-4-9-9-9Z" />
    <circle cx="7.5" cy="11" r="1" />
    <circle cx="10.5" cy="7" r="1" />
    <circle cx="15.5" cy="8" r="1" />
  </svg>
);

/** Anneau de pétales : le menu des formats. */
export const RingIcon = () => (
  <svg {...nav}>
    <circle cx="12" cy="12" r="2.5" />
    <circle cx="12" cy="4" r="1.6" />
    <circle cx="19" cy="9" r="1.6" />
    <circle cx="17" cy="18" r="1.6" />
    <circle cx="7" cy="18" r="1.6" />
    <circle cx="5" cy="9" r="1.6" />
  </svg>
);

export const FolderIcon = () => (
  <svg {...nav}>
    <path d="M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5Z" />
  </svg>
);

export const ToolsIcon = () => (
  <svg {...nav}>
    <path d="M4 7h10M18 7h2M4 17h2M10 17h10" />
    <circle cx="16" cy="7" r="2" />
    <circle cx="8" cy="17" r="2" />
  </svg>
);

export const UserIcon = () => (
  <svg {...nav}>
    <circle cx="12" cy="8.5" r="3.5" />
    <path d="M5 20a7 7 0 0 1 14 0" />
  </svg>
);

export const SystemIcon = () => (
  <svg {...nav}>
    <rect x="3" y="4" width="18" height="12" rx="1.5" />
    <path d="M9 20h6M12 16v4" />
  </svg>
);

export const InfoIcon = () => (
  <svg {...nav}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5M12 8h.01" />
  </svg>
);

/** Conversion : un format devient un autre. */
export const ConvertIcon = () => (
  <svg {...base}>
    <path d="M4 8h13l-3-3M20 16H7l3 3" />
  </svg>
);

export const ArrowRightIcon = () => (
  <svg {...nav} width={14} height={14}>
    <path d="M5 12h14M13 6l6 6-6 6" />
  </svg>
);

export const ExternalIcon = () => (
  <svg {...nav} width={13} height={13}>
    <path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" />
  </svg>
);
