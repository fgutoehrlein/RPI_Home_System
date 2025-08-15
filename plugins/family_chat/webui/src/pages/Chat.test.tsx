/** @vitest-environment jsdom */
import { render, screen, fireEvent } from '@testing-library/react';
import { vi, expect, test } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { MemoryRouter } from 'react-router-dom';

vi.mock('../components/Layout', () => ({ default: ({ children }: any) => <div>{children}</div> }));
vi.mock('../components/MessageList', () => ({
  default: ({ messages }: any) => (
    <div>
      {messages.map((m: any) => (
        <div key={m.id}>{m.text_md}</div>
      ))}
    </div>
  ),
}));

import Chat from './Chat';
import { api } from '../lib/api';

function defer<T>() {
  let resolve: (v: T) => void = () => {};
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

test('sending a message shows it immediately', async () => {
  const getMessages = defer<any[]>();
  vi.spyOn(api, 'getMessages').mockReturnValue(getMessages.promise as any);
  vi.spyOn(api, 'sendMessage').mockResolvedValue({
    id: 'temp',
    room_id: '1',
    text_md: 'hello',
    user: { id: 'u1', username: 'me', display_name: 'Me' },
    created_at: 'now',
  } as any);

  render(
    <MemoryRouter>
      <Chat />
    </MemoryRouter>
  );

  const input = screen.getByPlaceholderText('Type a message');
  fireEvent.change(input, { target: { value: 'hello' } });
  fireEvent.keyDown(input, { key: 'Enter', code: 'Enter', charCode: 13 });

  expect(api.sendMessage).toHaveBeenCalled();
  expect(document.body.innerHTML).toContain('hello');

  // Now resolve initial fetch which should not remove the existing message
  getMessages.resolve([]);
  await Promise.resolve();

  expect(screen.getByText('hello')).toBeInTheDocument();
});
