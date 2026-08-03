import { useEffect, useState } from 'react';

/** True on narrow (phone) viewports. Drives the single-column mobile layout. */
export function useIsMobile(breakpoint = 640): boolean {
  const query = `(max-width: ${breakpoint}px)`;
  const [mobile, setMobile] = useState(() =>
    typeof window !== 'undefined' && typeof window.matchMedia === 'function'
      ? window.matchMedia(query).matches
      : false,
  );
  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
    const mq = window.matchMedia(query);
    const onChange = () => setMobile(mq.matches);
    onChange();
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, [query]);
  return mobile;
}
