const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: __dirname,
  retries: 1,
  timeout: 30000,
  use: {
    headless: true,
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  reporter: [['html', { outputFolder: 'playwright-report', open: 'never' }], ['line']],
});
