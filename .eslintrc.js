module.exports = {
  extends: "erb",
  plugins: ["@typescript-eslint"],
  rules: {
    // A temporary hack related to IDE not resolving correct package.json
    "import/no-extraneous-dependencies": "warn",
    "react/react-in-jsx-scope": "off",
    "react/jsx-filename-extension": "off",
    "import/extensions": "off",
    "import/no-unresolved": "off",
    "import/no-import-module-exports": "off",
    "no-shadow": "off",
    "@typescript-eslint/no-shadow": "warn",
    "no-unused-vars": "off",
    "@typescript-eslint/no-unused-vars": "warn",
    "react/prop-types": "warn",
    "react/require-default-props": "off",
    "no-alert": "warn",
    "no-restricted-syntax": "warn",
    "jsx-a11y/click-events-have-key-events": "warn",
    "jsx-a11y/no-static-element-interactions": "warn",
    "jsx-a11y/label-has-associated-control": [2, {labelAttributes: ["htmlFor"]}],
    "promise/always-return": "off",
  },
  parserOptions: {
    ecmaVersion: 2022,
    sourceType: "module",
  },
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
