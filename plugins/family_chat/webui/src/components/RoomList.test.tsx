/** @vitest-environment jsdom */
import { render, screen, fireEvent } from '@testing-library/react';
import RoomList from './RoomList';
import { vi, beforeEach, expect, test } from 'vitest';
import { api } from '../lib/api';
import '@testing-library/jest-dom/vitest';
import { MemoryRouter } from 'react-router-dom';

vi.mock('../lib/api');

const mockedApi = api as any;

beforeEach(() => {
  mockedApi.listRooms.mockResolvedValue([]);
  mockedApi.createRoom.mockResolvedValue({ id: 'r1', name: 'Test' });
});

test('can create a room from the sidebar', async () => {
  render(
    <MemoryRouter>
      <RoomList />
    </MemoryRouter>
  );

  const btn = await screen.findByRole('button', { name: /new room/i });
  fireEvent.click(btn);
  const input = await screen.findByPlaceholderText(/room name/i);
  fireEvent.change(input, { target: { value: 'Test' } });
  fireEvent.click(screen.getByRole('button', { name: /create/i }));

  expect(mockedApi.createRoom).toHaveBeenCalledWith('Test');
  expect(await screen.findByText('Test')).toBeInTheDocument();
});
