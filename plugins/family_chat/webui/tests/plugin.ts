import { spawn } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import net from 'net';
import { setTimeout as delay } from 'timers/promises';

async function getPort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.listen(0, () => {
      const port = (srv.address() as any).port;
      srv.close(() => resolve(port));
    });
    srv.on('error', reject);
  });
}

async function waitReady(base: string, timeout = 10000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    try {
      const res = await fetch(base);
      if (res.ok) return;
    } catch {}
    await delay(100);
  }
  throw new Error('server not ready');
}

export async function withRunningPlugin<T>(fn: (ctx: { port: number; baseUrl: string }) => Promise<T>) {
  const port = await getPort();
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-'));
  const dataDir = path.join(tmp, 'data');
  fs.mkdirSync(dataDir);
  const configPath = path.join(tmp, 'config.toml');
  const config = `[bootstrap]\nusername = "admin"\npassword = "admin"\n\n[server]\nport = ${port}\n\n[logging]\nenabled = false\n`;
  fs.writeFileSync(configPath, config);
  const bin = path.resolve(__dirname, '../../../../target/release/family_chat');
  const logPath = path.join(process.cwd(), 'e2e-agent-plugin.log');
  const logStream = fs.createWriteStream(logPath);
  const proc = spawn(bin, ['--config', configPath], {
    env: { ...process.env, DATA_DIR: dataDir },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  proc.stdout.pipe(logStream);
  proc.stderr.pipe(logStream);

  const baseUrl = `http://127.0.0.1:${port}`;
  try {
    await waitReady(baseUrl);
    return await fn({ port, baseUrl });
  } finally {
    proc.kill();
    logStream.end();
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}
