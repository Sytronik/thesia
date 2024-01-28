module.exports = {
  extends: "erb",
  plugins: ["@typescript-eslint"],
  rules: {
    // A temporary hack related to IDE not resolving correct package.json
    "import/no-extraneous-dependencies": "off",
    "react/react-in-jsx-scope": "off",
    "react/jsx-filename-extension": "off",
    "import/extensions": "off",
    "import/no-unresolved": "off",
    "import/no-import-module-exports": "off",
    "no-shadow": "off",
    "@typescript-eslint/no-shadow": "error",
    "no-unused-vars": "off",
    "@typescript-eslint/no-unused-vars": ["error", {destructuredArrayIgnorePattern: "^_"}],
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
    camelcase: ["error", {allow: ["^dB_", "_dB$", "amp_range"]}],
  },
  parserOptions: {
    ecmaVersion: 2022,
    sourceType: "module",
  },
  overrides: [
    {
      files: ["*.ts", "*.tsx"],
      rules: {
        "no-undef": "off",
      },
    },
  ],
  settings: {
    "import/resolver": {
      // See https://github.com/benmosher/eslint-plugin-import/issues/1396#issuecomment-575727774 for line below
      node: {},
      webpack: {
        config: require.resolve("./.erb/configs/webpack.config.eslint.ts"),
      },
      typescript: {},
    },
    "import/parsers": {
      "@typescript-eslint/parser": [".ts", ".tsx"],
    },
  },
};
