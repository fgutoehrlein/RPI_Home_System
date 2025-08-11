import { useState } from 'react';
import { api } from '../lib/api';

export default function Bootstrap() {
  const [passphrase, setPassphrase] = useState('');

  async function submit() {
    await api.bootstrap({ passphrase, users: [] });
    window.location.href = '/login';
  }

  return (
    <div className="p-4 max-w-sm mx-auto">
      <h1 className="mb-2 text-xl">Bootstrap</h1>
      <input
        type="password"
        className="mb-2 w-full rounded border p-2"
        placeholder="Passphrase"
        value={passphrase}
        onChange={(e) => setPassphrase(e.target.value)}
      />
      <button className="rounded bg-blue-600 px-4 py-2 text-white" onClick={submit}>
        Save
      </button>
    </div>
  );
}
