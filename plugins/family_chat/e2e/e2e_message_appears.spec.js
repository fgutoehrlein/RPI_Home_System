const { test, expect } = require('@playwright/test');
const { spawn } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { setTimeout: delay } = require('timers/promises');

let proc;
let port;
let dataDir;

async function waitForServer(url) {
  for (let i = 0; i < 50; i++) {
    try {
      const res = await fetch(url);
      if (res.ok) return true;
    } catch (e) {}
    await delay(100);
  }
  throw new Error('server did not start');
}

test.beforeAll(async () => {
  port = 18787 + Math.floor(Math.random() * 1000);
  dataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-data-'));
  const bin = path.resolve(__dirname, '../../../target/release/family_chat');
  proc = spawn(bin, [], {
    env: { ...process.env, BIND: `127.0.0.1:${port}`, DATA_DIR: dataDir, RUST_LOG: 'off' },
    stdio: 'inherit',
  });

  await waitForServer(`http://127.0.0.1:${port}/`);

  await fetch(`http://127.0.0.1:${port}/api/bootstrap`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      passphrase: 'admin',
      users: [
        { username: 'admin', display_name: 'Admin', admin: true },
        { username: 'user', display_name: 'User', admin: false },
      ],
    }),
  });
});

test.afterAll(async () => {
  proc.kill();
  fs.rmSync(dataDir, { recursive: true, force: true });
});

test('message is displayed after sending', async ({ page }) => {
  const base = `http://127.0.0.1:${port}`;
  await page.goto(`${base}/login`);
  await page.fill('[data-testid="login-username"]', 'admin');
  await page.fill('[data-testid="login-password"]', 'admin');
  await page.click('[data-testid="login-submit"]');
  await page.waitForSelector('[data-testid="composer-input"]');

  await page.click('[data-testid="new-room-button"]');
  const roomName = `e2e-room-${Date.now()}`;
  await page.fill('[data-testid="new-room-name"]', roomName);
  await page.click('[data-testid="new-room-submit"]');
  await page.waitForSelector('[data-testid="composer-input"]');

  const msg = `Hello from E2E ${Date.now()}`;
  await page.fill('[data-testid="composer-input"]', msg);
  await page.keyboard.press('Enter');

  await expect(page.locator('[data-testid="composer-input"]')).toHaveValue('');
  await expect(
    page.locator('[data-testid="message-list"] [data-testid="message-text"]').last()
  ).toHaveText(msg);
});
