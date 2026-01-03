import js from "@eslint/js";
import tseslint from "@typescript-eslint/eslint-plugin";
import tsparser from "@typescript-eslint/parser";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import jsxA11y from "eslint-plugin-jsx-a11y";
import promise from "eslint-plugin-promise";

export default [
  js.configs.recommended,
  {
    ignores: [
      // Dependencies
      "node_modules/**",
      "package-lock.json",
      // Build outputs
      "dist/**",
      "target/**",
      "src-wasm/pkg/**",
      // Generated files
      "**/*.scss.d.ts",
      "**/*.d.ts",
      "!src/**/*.d.ts",
      // Sample/test files
      "samples/**",
      "public/**",
      // Rust source files
      "**/*.rs",
      "src-tauri/**",
      "src-wasm/**",
      // Config files
      "Cargo.toml",
      "Cargo.lock",
      // Logs
      "**/*.log",
      "log.txt",
      // Other
      "README.md",
      "index.html",
      ".gitignore",
    ],
  },
  {
    files: ["**/*.{js,jsx,ts,tsx}"],
    languageOptions: {
      parser: tsparser,
      parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        ecmaFeatures: {
          jsx: true,
        },
      },
      globals: {
        window: "readonly",
        document: "readonly",
        console: "readonly",
        process: "readonly",
      },
    },
    plugins: {
      "@typescript-eslint": tseslint,
      react: react,
      "react-hooks": reactHooks,
      "jsx-a11y": jsxA11y,
      promise: promise,
    },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react/react-in-jsx-scope": "off",
      "react/jsx-filename-extension": "off",
      //   "import/extensions": "off",
      //   "import/no-unresolved": "off",
      //   "import/no-import-module-exports": "off",
      "no-undef": "off",
      "no-shadow": "off",
      "@typescript-eslint/no-shadow": "error",
      "no-unused-vars": "off",
      "react-hooks/exhaustive-deps": "warn",
      "react/prop-types": "warn",
      "react/require-default-props": "off",
      "no-alert": "warn",
      "no-restricted-syntax": "warn",
      "no-restricted-exports": 0,
      "jsx-a11y/click-events-have-key-events": "warn",
      "jsx-a11y/no-static-element-interactions": "warn",
      "jsx-a11y/label-has-associated-control": [2, {labelAttributes: ["htmlFor"]}],
      "jsx-a11y/control-has-associated-label": "warn",
      "promise/always-return": "off",
      "@typescript-eslint/no-unused-vars": ["warn", {argsIgnorePattern: "^_"}],
    },
    settings: {
      react: {
        version: "detect",
      },
    },
  },
];
