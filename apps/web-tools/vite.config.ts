import { defineConfig, type Plugin } from "vite";
import { cloudflare } from "@cloudflare/vite-plugin";
import react from "@vitejs/plugin-react";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";
import { spawn } from "child_process";
import path from "path";

function wasmHotReload(): Plugin {
  return {
    name: "wasm-hot-reload",
    apply: "serve",
    configureServer(server) {
      const root = server.config.root;
      const workspaceRoot = path.resolve(root, "../..");

      const rustDirs = [
        "bindings/web-simulator/src",
        "bindings/trajectory-recorder/src",
        "ossm/src",
        "drivers/sim-motor/src",
        "crates/pattern-engine/src",
      ].map((d) => path.resolve(workspaceRoot, d));

      for (const dir of rustDirs) {
        server.watcher.add(dir);
      }

      let building = false;
      let pendingBuild = false;

      function rebuild() {
        if (building) {
          pendingBuild = true;
          return;
        }

        building = true;
        console.log("\x1b[36m[wasm]\x1b[0m Rebuilding...");

        const proc = spawn(
          "just",
          ["build-wasm"],
          { cwd: workspaceRoot, stdio: "inherit" },
        );

        proc.on("close", (code: number | null) => {
          building = false;

          if (code === 0) {
            console.log("\x1b[36m[wasm]\x1b[0m Ready, restarting dev server...");
            server.restart();
          } else {
            console.error("\x1b[31m[wasm]\x1b[0m Build failed");
          }

          if (pendingBuild) {
            pendingBuild = false;
            rebuild();
          }
        });
      }

      let debounceTimer: ReturnType<typeof setTimeout> | null = null;

      server.watcher.on("change", (file) => {
        if (!file.endsWith(".rs")) return;

        if (debounceTimer) clearTimeout(debounceTimer);
        debounceTimer = setTimeout(rebuild, 300);
      });
    },
  };
}

export default defineConfig({
  build: {
    target: "esnext",
  },
  optimizeDeps: {
    exclude: ["@ossm-rs/web-simulator", "@ossm-rs/trajectory-recorder"],
  },
  server: {
    watch: {
      ignored: [
        "**/bindings/web-simulator/pkg/**",
        "**/bindings/trajectory-recorder/pkg/**",
      ],
    },
  },
  plugins: [react(), wasm(), topLevelAwait(), wasmHotReload(), cloudflare()],
});
