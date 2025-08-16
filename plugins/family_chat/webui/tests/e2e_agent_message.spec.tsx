/** @vitest-environment jsdom */
import { withRunningPlugin } from './plugin';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { fetch, Headers, Request, Response } from 'undici';
import WS from 'ws';
import { webcrypto } from 'crypto';
import { describe, it, expect } from 'vitest';
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

function polyfill() {
  (globalThis as any).fetch = fetch as any;
  (globalThis as any).Headers = Headers as any;
  (globalThis as any).Request = Request as any;
  (globalThis as any).Response = Response as any;
  (globalThis as any).WebSocket = WS as any;
  if (!(globalThis as any).crypto) {
    (globalThis as any).crypto = webcrypto as any;
  }
  (globalThis as any).ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

describeIf('agent e2e message flow', () => {
  it('sends and displays a message', async () => {
    await withRunningPlugin(async ({ baseUrl }) => {
      polyfill();
      (globalThis as any).__FC_BASE__ = baseUrl;
      (window as any).__FC_BASE__ = baseUrl;
      sessionStorage.clear();
      window.history.pushState({}, '', '/login');
      const { default: App } = await import('../src/App');
      render(<App />);

      fireEvent.change(screen.getByTestId('login-username'), {
        target: { value: 'admin' },
      });
      fireEvent.change(screen.getByTestId('login-password'), {
        target: { value: 'admin' },
      });
      fireEvent.click(screen.getByTestId('login-submit'));

      await screen.findByTestId('new-room-button');

      fireEvent.click(screen.getByTestId('new-room-button'));
      const roomName = `e2e-room-${Date.now()}`;
      fireEvent.change(screen.getByTestId('new-room-name'), {
        target: { value: roomName },
      });
      fireEvent.click(screen.getByTestId('new-room-submit'));

      const composer = await screen.findByTestId('composer-input');
      const msg = `Hello from Agent E2E ${Date.now()}`;
      fireEvent.change(composer, { target: { value: msg } });
      fireEvent.keyDown(composer, { key: 'Enter', code: 'Enter', charCode: 13 });

      await waitFor(() => expect(composer).toHaveValue(''));

      await screen.findByText(msg);
      await waitFor(() => {
        expect(screen.queryAllByText(msg).length).toBe(1);
      });
    });
  }, 60000);
});
