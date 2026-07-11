import js from "@eslint/js";
import tseslint from "typescript-eslint";
import globals from "globals";

export default tseslint.config(
  {
    ignores: ["dist/", "src-tauri/", "node_modules/", "coverage/"],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    // browser-runner is a standalone Node ESM script (Playwright), not part of the app bundle.
    files: ["browser-runner/**/*.{js,mjs}"],
    languageOptions: {
      sourceType: "module",
      globals: globals.node,
    },
  },
);
