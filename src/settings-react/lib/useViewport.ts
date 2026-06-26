import { useEffect, useState } from "react";

export function useViewport() {
  const [width, setWidth] = useState<number>(() =>
    typeof window === "undefined" ? 1024 : window.innerWidth,
  );

  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    onResize();
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return width;
}

export function useIsCompact(breakpoint = 860) {
  return useViewport() < breakpoint;
}
