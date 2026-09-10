import { Hono } from "hono";
import type { R2Bucket } from "@cloudflare/workers-types";
import { handleGithubAsset } from "./github-asset";
import {
  handleListPrFirmware,
  handleGetPrFirmware,
} from "./pr-firmware";

interface Env {
  ASSETS: Fetcher;
  FIRMWARE_BUCKET: R2Bucket;
}

const app = new Hono<{ Bindings: Env }>();

app.get("/api/github-asset/:id{\\d+}", (c) => {
  return handleGithubAsset(c.req.param("id"));
});

app.get("/api/pr-firmware/:pr{\\d+}", (c) => {
  return handleListPrFirmware(c.env.FIRMWARE_BUCKET, c.req.param("pr"));
});

app.get("/api/pr-firmware/:pr{\\d+}/:board", (c) => {
  return handleGetPrFirmware(
    c.env.FIRMWARE_BUCKET,
    c.req.param("pr"),
    c.req.param("board"),
  );
});

app.all("*", (c) => {
  return c.env.ASSETS.fetch(c.req.raw);
});

export default app;
