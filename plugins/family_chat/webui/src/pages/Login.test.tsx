/** @vitest-environment jsdom */
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import Login from './Login';
import { api } from '../lib/api';
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

const navigate = vi.fn();
vi.mock('react-router-dom', () => ({
  useNavigate: () => navigate,
}));

vi.spyOn(api, 'login');

describe('Login', () => {
  beforeEach(() => {
    (api.login as any).mockResolvedValue({ token: 'abc' });
    sessionStorage.clear();
    navigate.mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('stores token and redirects on success', async () => {
    render(<Login />);
    fireEvent.change(screen.getByPlaceholderText('Username'), { target: { value: 'alice' } });
    fireEvent.change(screen.getByPlaceholderText('Passphrase'), { target: { value: 'pw' } });
    fireEvent.click(screen.getByRole('button', { name: 'Login' }));
    await waitFor(() => expect(sessionStorage.getItem('fc_token')).toBe('abc'));
    expect(navigate).toHaveBeenCalledWith('/room/1');
  });
});
