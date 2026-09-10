import { useState, useEffect } from "react";

export interface ReleaseAsset {
  id: number;
  name: string;
  browser_download_url: string;
  size: number;
}

export interface Release {
  tag_name: string;
  name: string;
  published_at: string;
  assets: ReleaseAsset[];
}

const RELEASES_URL = "https://api.github.com/repos/ossm-rs/ossm/releases";

export function useGithubReleases() {
  const [releases, setReleases] = useState<Release[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function fetchReleases() {
      try {
        const response = await fetch(RELEASES_URL);
        if (!response.ok) {
          throw new Error(`GitHub API returned ${response.status}`);
        }
        const data: Release[] = await response.json();
        if (!cancelled) {
          // Only include releases that have .bin assets
          const withBinaries = data.filter((r) =>
            r.assets.some((a) => a.name.endsWith(".bin")),
          );
          setReleases(withBinaries);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to fetch releases");
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    fetchReleases();
    return () => {
      cancelled = true;
    };
  }, []);

  return { releases, loading, error };
}
