export async function handleGithubAsset(id: string): Promise<Response> {
  const upstream = await fetch(
    `https://api.github.com/repos/ossm-rs/ossm/releases/assets/${id}`,
    {
      headers: {
        Accept: "application/octet-stream",
        "User-Agent": "ossm-web-tools",
      },
      redirect: "follow",
    },
  );

  if (!upstream.ok) {
    return Response.json(
      { error: `GitHub returned ${upstream.status}` },
      { status: upstream.status },
    );
  }

  return new Response(upstream.body, {
    headers: {
      "Content-Type": "application/octet-stream",
      "Cache-Control": "public, max-age=3600",
    },
  });
}
