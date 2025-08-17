import { access, writeFile } from 'fs/promises';
import { constants } from 'fs';
import { spawn } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const bin = resolve(__dirname, '../../../../target/release/family_chat');
const report = resolve(process.cwd(), 'e2e-agent-report.xml');

try {
  await access(bin, constants.X_OK);
} catch {
  await writeFile(report, '<testsuites></testsuites>');
  console.log('family_chat binary not found; skipping e2e agent tests');
  process.exit(0);
}

const args = [
  'run',
  'tests/e2e_agent_message.spec.tsx',
  '--environment',
  'jsdom',
  '--reporter',
  'junit',
  '--outputFile',
  report,
  '--passWithNoTests',
];

const child = spawn('vitest', args, { stdio: 'inherit' });
child.on('exit', (code) => process.exit(code ?? 1));
