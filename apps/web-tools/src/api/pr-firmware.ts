import type { R2Bucket } from "@cloudflare/workers-types";

export async function handleListPrFirmware(
  bucket: R2Bucket,
  pr: string,
): Promise<Response> {
  const list = await bucket.list({ prefix: `pr-${pr}/` });

  const boards = list.objects
    .filter((obj) => obj.key.endsWith(".bin"))
    .map((obj) => {
      const filename = obj.key.split("/").pop()!;
      return {
        board: filename.replace(/\.bin$/, ""),
        size: obj.size,
      };
    });

  return Response.json(boards, {
    headers: { "Cache-Control": "no-cache" },
  });
}

export async function handleGetPrFirmware(
  bucket: R2Bucket,
  pr: string,
  board: string,
): Promise<Response> {
  const object = await bucket.get(`pr-${pr}/${board}.bin`);

  if (!object) {
    return Response.json(
      { error: `No firmware found for PR #${pr}, board "${board}"` },
      { status: 404 },
    );
  }

  return new Response(object.body as ReadableStream, {
    headers: {
      "Content-Type": "application/octet-stream",
      "Content-Length": object.size.toString(),
      "Cache-Control": "no-cache",
    },
  });
}
