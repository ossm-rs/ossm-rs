import { useState, useEffect } from "react";

export interface PrBoard {
  board: string;
  size: number;
}

export function usePrFirmware(pr: string | null) {
  const [boards, setBoards] = useState<PrBoard[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!pr) return;

    let cancelled = false;
    setLoading(true);
    setError(null);

    async function fetchBoards() {
      try {
        const response = await fetch(`/api/pr-firmware/${pr}`);
        if (!response.ok) {
          throw new Error(`Failed to fetch PR firmware: ${response.status}`);
        }
        const data: PrBoard[] = await response.json();
        if (!cancelled) {
          setBoards(data);
        }
      } catch (err) {
        if (!cancelled) {
          setError(
            err instanceof Error ? err.message : "Failed to fetch PR firmware",
          );
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    fetchBoards();
    return () => {
      cancelled = true;
    };
  }, [pr]);

  return { boards, loading, error };
}
