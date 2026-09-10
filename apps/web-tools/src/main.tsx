import { createRoot } from "react-dom/client";
import { BrowserRouter, Routes, Route, Navigate } from "react-router";
import "@radix-ui/themes/styles.css";
import "./global.css";
import { AppearanceProvider } from "./AppearanceProvider";
import { SimulatorProvider } from "./SimulatorProvider";
import Layout from "./Layout";
import DiagramPage from "./pages/DiagramPage";
import FlasherPage from "./pages/FlasherPage";
import GraphPage from "./pages/GraphPage";
import SimulatorPage from "./pages/SimulatorPage";

createRoot(document.getElementById("root")!).render(
  <BrowserRouter>
    <AppearanceProvider>
      <SimulatorProvider
        fallback={
          <div
            style={{
              height: "100vh",
              backgroundColor:
                localStorage.getItem("theme") === "dark" ||
                (localStorage.getItem("theme") == null &&
                  window.matchMedia("(prefers-color-scheme: dark)").matches)
                  ? "#111113"
                  : "#ffffff",
            }}
          />
        }
      >
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<Navigate to="/simulator" replace />} />
            <Route path="simulator" element={<SimulatorPage />} />
            <Route path="graph" element={<GraphPage />} />
            <Route path="diagram" element={<DiagramPage />} />
            <Route path="firmware" element={<FlasherPage />}>
              <Route path="release/:release/:board" element={null} />
              <Route path="release/:release" element={null} />
              <Route path="release" element={null} />
              <Route path="pr/:pr/:board" element={null} />
              <Route path="pr/:pr" element={null} />
            </Route>
            <Route path="flasher" element={<FlasherPage />} />
          </Route>
        </Routes>
      </SimulatorProvider>
    </AppearanceProvider>
  </BrowserRouter>,
);
