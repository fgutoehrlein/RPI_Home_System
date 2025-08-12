import { useStore } from '../lib/store';

export default function Settings() {
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  return (
    <div className="p-4">
      <h1 className="mb-2 text-xl">Settings</h1>
      <label className="block mb-2">
        Theme:
        <select
          className="ml-2 rounded border p-1"
          value={theme}
          onChange={(e) => setTheme(e.target.value as any)}
        >
          <option value="light">Light</option>
          <option value="dark">Dark</option>
          <option value="system">System</option>
        </select>
      </label>
    </div>
  );
}
