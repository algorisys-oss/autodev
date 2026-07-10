import { defineConfig } from "vitest/config";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  // Resolve Solid's browser/development build so components render under jsdom (the default
  // picks the SSR build, which throws "Client-only API called on the server side").
  resolve: {
    conditions: ["development", "browser"],
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
