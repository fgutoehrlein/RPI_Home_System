/** @vitest-environment jsdom */
import { withRunningPlugin } from './plugin';
import { render, fireEvent, waitFor } from '@testing-library/react';
import { JSDOM } from 'jsdom';
import { fetch, Headers, Request, Response } from 'undici';
import WS from 'ws';
import { webcrypto } from 'crypto';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import fs from 'fs';
import path from 'path';

const bin = path.resolve(
  __dirname,
  '../../../../target/release/family_chat',
);
const describeIf = (() => {
  try {
    fs.accessSync(bin, fs.constants.X_OK);
    return describe;
  } catch {
    return describe.skip;
  }
})();

function polyfill(target: any) {
  target.fetch = fetch as any;
  target.Headers = Headers as any;
  target.Request = Request as any;
  target.Response = Response as any;
  target.WebSocket = WS as any;
  if (!target.crypto) target.crypto = webcrypto as any;
  target.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

async function createClient(baseUrl: string) {
  const dom = new JSDOM('<!doctype html><html><body></body></html>', {
    url: baseUrl + '/login',
  });
  const win = dom.window as any;
  polyfill(win);
  const prev = {
    window: globalThis.window,
    document: globalThis.document,
    sessionStorage: globalThis.sessionStorage,
    navigator: globalThis.navigator,
  };
  (globalThis as any).window = win;
  (globalThis as any).document = win.document;
  (globalThis as any).sessionStorage = win.sessionStorage;
  (globalThis as any).navigator = win.navigator;
  vi.resetModules();
  const { default: App } = await import('../src/App');
  const rootEl = win.document.createElement('div');
  win.document.body.appendChild(rootEl);
  const utils = render(<App />, { container: rootEl });
  (globalThis as any).window = prev.window;
  (globalThis as any).document = prev.document;
  (globalThis as any).sessionStorage = prev.sessionStorage;
  (globalThis as any).navigator = prev.navigator;
  return { window: win, ...utils };
}

async function runInClient<T>(client: any, fn: () => Promise<T> | T): Promise<T> {
  const prev = {
    window: globalThis.window,
    document: globalThis.document,
    sessionStorage: globalThis.sessionStorage,
    navigator: globalThis.navigator,
  };
  (globalThis as any).window = client.window;
  (globalThis as any).document = client.window.document;
  (globalThis as any).sessionStorage = client.window.sessionStorage;
  (globalThis as any).navigator = client.window.navigator;
  try {
    return await fn();
  } finally {
    (globalThis as any).window = prev.window;
    (globalThis as any).document = prev.document;
    (globalThis as any).sessionStorage = prev.sessionStorage;
    (globalThis as any).navigator = prev.navigator;
  }
}

describeIf('agent e2e realtime two clients', () => {
  it('propagates messages between two clients', async () => {
    const errors: unknown[] = [];
    const originalError = console.error;
    console.error = (...args: unknown[]) => {
      errors.push(args);
      originalError(...args);
    };

    try {
      await withRunningPlugin(async ({ baseUrl }) => {
        (globalThis as any).__FC_BASE__ = baseUrl;

        const clientA = await createClient(baseUrl);
        const clientB = await createClient(baseUrl);

        await runInClient(clientA, async () => {
          fireEvent.change(clientA.getByTestId('login-username'), {
            target: { value: 'admin' },
          });
          fireEvent.change(clientA.getByTestId('login-password'), {
            target: { value: 'admin' },
          });
          fireEvent.click(clientA.getByTestId('login-submit'));
          await clientA.findByTestId('new-room-button');
        });

        const adminToken = clientA.window.sessionStorage.getItem('fc_token')!;
        await fetch(`${baseUrl}/api/admin/users`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          body: JSON.stringify({ username: 'userb', display_name: 'User B' }),
        });

        const roomName = `e2e-realtime-${Date.now()}`;
        await runInClient(clientA, async () => {
          fireEvent.click(clientA.getByTestId('new-room-button'));
          fireEvent.change(await clientA.findByTestId('new-room-name'), {
            target: { value: roomName },
          });
          fireEvent.click(clientA.getByTestId('new-room-submit'));
          await clientA.findByTestId('composer-input');
        });

        await runInClient(clientB, async () => {
          fireEvent.change(clientB.getByTestId('login-username'), {
            target: { value: 'userb' },
          });
          fireEvent.change(clientB.getByTestId('login-password'), {
            target: { value: 'admin' },
          });
          fireEvent.click(clientB.getByTestId('login-submit'));
          await clientB.findByTestId('new-room-button');
          const btn = await clientB.findByText(roomName);
          fireEvent.click(btn);
          await clientB.findByTestId('composer-input');
        });

        await runInClient(clientA, async () => {
          await waitFor(
            () =>
              expect(clientA.getByTestId('ws-status').textContent).toBe(
                'connected',
              ),
            { timeout: 10000 },
          );
        });
        await runInClient(clientB, async () => {
          await waitFor(
            () =>
              expect(clientB.getByTestId('ws-status').textContent).toBe(
                'connected',
              ),
            { timeout: 10000 },
          );
        });

        const msgA = `A->B hello ${Date.now()}`;
        await runInClient(clientA, async () => {
          const composer = await clientA.findByTestId('composer-input');
          fireEvent.change(composer, { target: { value: msgA } });
          fireEvent.keyDown(composer, {
            key: 'Enter',
            code: 'Enter',
            charCode: 13,
          });
          await waitFor(() => expect(composer).toHaveValue(''), {
            timeout: 10000,
          });
          await clientA.findByText(msgA, undefined, { timeout: 10000 });
          await waitFor(
            () => expect(clientA.queryAllByText(msgA).length).toBe(1),
            { timeout: 10000 },
          );
        });

        await runInClient(clientB, async () => {
          await clientB.findByText(msgA, undefined, { timeout: 10000 });
          await waitFor(
            () => expect(clientB.queryAllByText(msgA).length).toBe(1),
            { timeout: 10000 },
          );
        });

        const msgB = `B->A hello ${Date.now()}`;
        await runInClient(clientB, async () => {
          const composer = await clientB.findByTestId('composer-input');
          fireEvent.change(composer, { target: { value: msgB } });
          fireEvent.keyDown(composer, {
            key: 'Enter',
            code: 'Enter',
            charCode: 13,
          });
          await waitFor(() => expect(composer).toHaveValue(''), {
            timeout: 10000,
          });
          await clientB.findByText(msgB, undefined, { timeout: 10000 });
          await waitFor(
            () => expect(clientB.queryAllByText(msgB).length).toBe(1),
            { timeout: 10000 },
          );
        });

        await runInClient(clientA, async () => {
          await clientA.findByText(msgB, undefined, { timeout: 10000 });
          await waitFor(
            () => expect(clientA.queryAllByText(msgB).length).toBe(1),
            { timeout: 10000 },
          );
        });
      });

      expect(errors).toHaveLength(0);
    } finally {
      console.error = originalError;
    }
  }, 120000);
});

