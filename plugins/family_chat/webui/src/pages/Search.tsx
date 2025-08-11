import { useState } from 'react';
import { api } from '../lib/api';
import { SearchResult } from '../lib/types';

export default function Search() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);

  async function submit() {
    const res = await api.search(query);
    setResults(res);
  }

  return (
    <div className="p-4">
      <h1 className="mb-2 text-xl">Search</h1>
      <input
        className="mb-2 w-full rounded border p-2"
        placeholder="Search"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <button className="rounded bg-blue-600 px-4 py-2 text-white" onClick={submit}>
        Go
      </button>
      <ul className="mt-4 space-y-2">
        {results.map((r) => (
          <li key={r.message.id} className="rounded border p-2">
            {r.message.text_md}
          </li>
        ))}
      </ul>
    </div>
  );
}
