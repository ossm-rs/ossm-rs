import { useState, useEffect } from "react";

export interface PrAuthor {
  username: string;
  avatar_url: string;
  html_url: string;
}

export interface PrInfo {
  number: number;
  title: string;
  body: string | null;
  author: PrAuthor;
}

export function useGithubPr(pr: string | null) {
  const [prInfo, setPrInfo] = useState<PrInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!pr) return;

    let cancelled = false;
    setLoading(true);
    setError(null);

    async function fetchPr() {
      try {
        const response = await fetch(
          `https://api.github.com/repos/ossm-rs/ossm/pulls/${pr}`,
        );
        if (!response.ok) {
          throw new Error(`GitHub API returned ${response.status}`);
        }
        const data = await response.json();
        if (!cancelled) {
          setPrInfo({
            number: data.number,
            title: data.title,
            body: data.body,
            author: {
              username: data.user.login,
              avatar_url: data.user.avatar_url,
              html_url: data.user.html_url,
            },
          });
        }
      } catch (err) {
        if (!cancelled) {
          setError(
            err instanceof Error ? err.message : "Failed to fetch PR info",
          );
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    fetchPr();
    return () => {
      cancelled = true;
    };
  }, [pr]);

  return { prInfo, loading, error };
}
