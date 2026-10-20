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
