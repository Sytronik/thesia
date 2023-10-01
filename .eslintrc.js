module.exports = {
  extends: "erb",
  rules: {
    // A temporary hack related to IDE not resolving correct package.json
    "import/no-extraneous-dependencies": "warn",
    "import/no-unresolved": "error",
    // Since React 17 and typescript 4.1 you can safely disable the rule
    "react/react-in-jsx-scope": "off",
    "react/prop-types": "warn",
    "react/require-default-props": "off",
    "no-alert": "warn",
    "no-restricted-syntax": "warn",
    "jsx-a11y/click-events-have-key-events": "warn",
    "jsx-a11y/no-static-element-interactions": "warn",
    "jsx-a11y/label-has-associated-control": [2, {labelAttributes: ["htmlFor"]}],
    "@typescript-eslint/no-shadow": "warn",
    "promise/always-return": "off",
  },
  parserOptions: {
    ecmaVersion: 2020,
    sourceType: "module",
    project: "./tsconfig.json",
    tsconfigRootDir: __dirname,
    createDefaultProgram: true,
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
