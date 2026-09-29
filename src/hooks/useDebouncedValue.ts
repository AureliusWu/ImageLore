import { useEffect, useState } from "react";
export function useDebouncedValue<T>(value: T, delay = 180) {
  const [out, setOut] = useState(value);
  useEffect(() => {
    const t = window.setTimeout(() => setOut(value), delay);
    return () => window.clearTimeout(t);
  }, [value, delay]);
  return out;
}
