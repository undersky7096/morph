import { useCallback, useState } from "react";

interface History<T> {
  past: T[];
  present: T;
  future: T[];
  /** Clé de fusion de la dernière modification, et son heure. */
  key: string | null;
  at: number;
}

const LIMIT = 100;
/** Modifications successives de même clé fusionnées en une seule étape (glisser un curseur). */
const MERGE_MS = 800;

/** État avec annuler/rétablir. */
export function useHistory<T>(initial: T) {
  const [h, setH] = useState<History<T>>({ past: [], present: initial, future: [], key: null, at: 0 });

  const update = useCallback((fn: (state: T) => T, mergeKey?: string) => {
    setH((h) => {
      const next = fn(h.present);
      if (next === h.present) return h;
      const now = Date.now();
      const merge = mergeKey !== undefined && mergeKey === h.key && now - h.at < MERGE_MS;
      return {
        past: merge ? h.past : [...h.past, h.present].slice(-LIMIT),
        present: next,
        future: [],
        key: mergeKey ?? null,
        at: now,
      };
    });
  }, []);

  const undo = useCallback(() => {
    setH((h) =>
      h.past.length
        ? { past: h.past.slice(0, -1), present: h.past[h.past.length - 1], future: [h.present, ...h.future], key: null, at: 0 }
        : h,
    );
  }, []);

  const redo = useCallback(() => {
    setH((h) =>
      h.future.length
        ? { past: [...h.past, h.present], present: h.future[0], future: h.future.slice(1), key: null, at: 0 }
        : h,
    );
  }, []);

  return { state: h.present, update, undo, redo, canUndo: h.past.length > 0, canRedo: h.future.length > 0 };
}
