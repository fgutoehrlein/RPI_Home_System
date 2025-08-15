import { useEffect, useState, FormEvent } from 'react';
import { api } from '../lib/api';
import { Room } from '../lib/types';
import Modal from './Modal';
import { useNavigate } from 'react-router-dom';

export default function RoomList() {
  const [rooms, setRooms] = useState<Room[]>([]);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const navigate = useNavigate();

  useEffect(() => {
    api.listRooms().then(setRooms).catch(console.error);
  }, []);

  async function createRoom(e: FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    try {
      const room = await api.createRoom(name.trim());
      setRooms((r) => [...r, room]);
      setOpen(false);
      setName('');
      navigate(`/room/${room.id}`);
    } catch (err) {
      console.error(err);
    }
  }

  return (
    <div>
      <h2 className="px-2 py-1 text-xs font-semibold">Rooms</h2>
      <ul className="space-y-1">
        {rooms.map((r) => (
          <li key={r.id} className="px-2 py-1 rounded hover:bg-gray-200 cursor-pointer">
            {r.name}
          </li>
        ))}
      </ul>
      <button
        data-testid="new-room-button"
        className="mt-2 px-2 py-1 text-sm text-blue-600"
        onClick={() => setOpen(true)}
      >
        New Room
      </button>
      <Modal open={open} onClose={() => setOpen(false)}>
        <form onSubmit={createRoom} className="space-y-2">
          <input
            data-testid="new-room-name"
            autoFocus
            placeholder="Room name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="border p-1"
          />
          <div className="text-right">
            <button
              type="submit"
              data-testid="new-room-submit"
              className="px-2 py-1 bg-blue-500 text-white rounded"
              disabled={!name.trim()}
            >
              Create
            </button>
          </div>
        </form>
      </Modal>
    </div>
  );
}
