/** @vitest-environment jsdom */
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import Login from './Login';
import { api } from '../lib/api';
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

vi.spyOn(api, 'login');

describe('Login', () => {
  beforeEach(() => {
    (api.login as any).mockResolvedValue({ token: 'abc' });
    sessionStorage.clear();
    Object.defineProperty(window, 'location', {
      value: { href: '' },
      writable: true,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('stores token and redirects on success', async () => {
    render(<Login />);
    fireEvent.change(screen.getByPlaceholderText('Username'), { target: { value: 'alice' } });
    fireEvent.change(screen.getByPlaceholderText('Passphrase'), { target: { value: 'pw' } });
    fireEvent.click(screen.getByRole('button', { name: 'Login' }));
    await waitFor(() => expect(sessionStorage.getItem('fc_token')).toBe('abc'));
    expect(window.location.href).toBe('/room/1');
  });
});
