module.exports = {
  env: {
    browser: true
  },
  plugins: ['html'],
  extends: [
    'standard'
  ],
  ignorePatterns: ['static/hoplitekb_wasm_rs.js'],
  rules: {
    semi: [2, 'always']
  }
};
