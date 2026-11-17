import js from "@eslint/js";
import globals from "globals";
import jsxA11y from "eslint-plugin-jsx-a11y-x";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

// eslint-plugin-jsx-a11y-x is the maintained fork of eslint-plugin-jsx-a11y
// (ESLint 10 support). It is registered under the original "jsx-a11y" name so
// rule ids and eslint-disable comments keep their familiar prefix.
const a11yRecommended = Object.fromEntries(
  Object.entries(jsxA11y.configs.recommended.rules).map(([rule, level]) => [
    rule.replace(/^jsx-a11y-x\//, "jsx-a11y/"),
    level,
  ]),
);

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "src/lib/generated"] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ["**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
    plugins: { "react-hooks": reactHooks },
    rules: reactHooks.configs.recommended.rules,
  },
  {
    files: ["**/*.{ts,tsx}"],
    plugins: { "jsx-a11y": jsxA11y },
    languageOptions: jsxA11y.configs.recommended.languageOptions ?? {},
    rules: a11yRecommended,
  },
);
