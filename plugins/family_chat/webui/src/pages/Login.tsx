import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../lib/api';
import { setToken } from '../lib/auth';

export default function Login() {
  const [username, setUsername] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const navigate = useNavigate();

  async function submit() {
    const res = await api.login(username, passphrase);
    setToken(res.token);
    navigate('/room/1');
  }

  return (
    <div className="p-4 max-w-sm mx-auto">
      <h1 className="mb-2 text-xl">Login</h1>
      <input
        className="mb-2 w-full rounded border p-2"
        placeholder="Username"
        value={username}
        onChange={(e) => setUsername(e.target.value)}
        data-testid="login-username"
      />
      <input
        type="password"
        className="mb-2 w-full rounded border p-2"
        placeholder="Passphrase"
        value={passphrase}
        onChange={(e) => setPassphrase(e.target.value)}
        data-testid="login-password"
      />
      <button
        className="rounded bg-blue-600 px-4 py-2 text-white"
        onClick={submit}
        data-testid="login-submit"
      >
        Login
      </button>
    </div>
  );
}
