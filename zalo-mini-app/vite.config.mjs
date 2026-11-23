import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import zaloMiniApp from "zmp-vite-plugin";

// The app lives in src/ (index.html too), as zmp-cli expects; the .env files
// (VITE_API_URL) stay at the project root.
export default defineConfig({
  root: "./src",
  envDir: "..",
  base: "",
  plugins: [zaloMiniApp(), react()],
  build: {
    assetsInlineLimit: 0,
    // zmp-cli deploys www/ at the project root.
    outDir: "../www",
    emptyOutDir: true,
  },
});
