import { AuthMe, LoginResponse, Message, Room, FileUploadResponse, SearchResult } from './types';
import { getToken, clearToken } from './auth';

function getBase(): string {
  return (
    (globalThis as any).__FC_BASE__ ||
    (import.meta as any).env?.VITE_FAMILY_CHAT_BASE ||
    ''
  );
}

function buildUrl(path: string): string {
  return `${getBase()}${path}`;
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(init.headers as any),
  };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const res = await globalThis.fetch(buildUrl(path), { ...init, headers });
  if (res.status === 401) {
    clearToken();
    window.location.href = '/login';
    throw new Error('unauthorized');
  }
  if (!res.ok) throw new Error(await res.text());
  return res.json() as Promise<T>;
}

export const api = {
  bootstrap(payload: any) {
    return request('/api/bootstrap', {
      method: 'POST',
      body: JSON.stringify(payload),
    });
  },
  login(username: string, passphrase: string) {
    return request<LoginResponse>('/api/login', {
      method: 'POST',
      body: JSON.stringify({ username, passphrase }),
    });
  },
  me() {
    return request<AuthMe>('/api/me');
  },
  listRooms() {
    return request<Room[]>('/api/rooms');
  },
  createRoom(name: string) {
    return request<Room>('/api/rooms', {
      method: 'POST',
      body: JSON.stringify({ name }),
    });
  },
  getMessages(roomId: string, before?: string, limit = 50) {
    const params = new URLSearchParams({ room_id: roomId, limit: String(limit) });
    if (before) params.append('before', before);
    return request<Message[]>(`/api/messages?${params.toString()}`);
  },
  sendMessage(payload: { room_id: string; text_md: string; reply_to?: string; attachments?: any[] }) {
    return request<Message>('/api/messages', {
      method: 'POST',
      body: JSON.stringify(payload),
    });
  },
  editMessage(id: string, text_md: string) {
    return request<Message>(`/api/messages/${id}`, {
      method: 'PATCH',
      body: JSON.stringify({ text_md }),
    });
  },
  deleteMessage(id: string) {
    return request<void>(`/api/messages/${id}`, { method: 'DELETE' });
  },
  uploadFile(file: File) {
    const form = new FormData();
    form.append('file', file);
    return globalThis.fetch(buildUrl('/api/files'), {
      method: 'POST',
      headers: { Authorization: `Bearer ${getToken()}` },
      body: form,
    }).then((r) => r.json() as Promise<FileUploadResponse>);
  },
  search(q: string, roomId?: string) {
    const params = new URLSearchParams({ q });
    if (roomId) params.append('room_id', roomId);
    return request<SearchResult[]>(`/api/search?${params.toString()}`);
  },
};
