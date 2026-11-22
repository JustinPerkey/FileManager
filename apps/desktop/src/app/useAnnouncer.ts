import { useCallback, useEffect, useState } from "react";

type AnnouncerState = { text: string; pending: string; id: number };

/** A live-region announcer that re-announces a repeated sentence: clear,
 *  then set on a frame scheduled only after the clear has committed. */
export function useAnnouncer(): { text: string; announce: (text: string) => void } {
  const [announcer, setAnnouncer] = useState<AnnouncerState>({ text: "", pending: "", id: 0 });
  // Clear now, set on a frame scheduled only after the clear has committed. A frame
  // requested from announce() itself could fire before React renders the clear (async
  // callers), batch both updates into one render and leave the DOM text unchanged.
  const announce = useCallback((text: string) => {
    setAnnouncer((a) => ({ text: "", pending: text, id: a.id + 1 }));
  }, []);
  const announceId = announcer.id;
  useEffect(() => {
    if (announceId === 0) return;
    const f = requestAnimationFrame(() => setAnnouncer((a) => ({ ...a, text: a.pending })));
    return () => cancelAnimationFrame(f);
  }, [announceId]);
  return { text: announcer.text, announce };
}
