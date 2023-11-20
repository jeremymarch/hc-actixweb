module.exports = {
  env: {
    browser: true
  },
  plugins: ['html'],
  extends: [
    'standard'
  ],
  ignorePatterns: ['static/hoplitekb_wasm_rs.js', 'hc-axum/dist/main.js'],
  rules: {
    semi: [2, 'always']
  }
};
