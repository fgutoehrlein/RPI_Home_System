import { create } from 'zustand';
import { User } from './types';

interface SessionState {
  user: User | null;
  token: string | null;
  setSession: (u: User, t: string) => void;
  clear: () => void;
}

interface UIState {
  theme: 'light' | 'dark' | 'system';
  setTheme: (t: UIState['theme']) => void;
}

interface ChatState {
  currentRoomId?: string;
  setRoom: (id?: string) => void;
  unread: Record<string, number>;
  setUnread: (id: string, count: number) => void;
}

export const useStore = create<SessionState & UIState & ChatState>((set) => ({
  // session
  user: null,
  token: null,
  setSession: (u, t) => set({ user: u, token: t }),
  clear: () => set({ user: null, token: null }),
  // ui
  theme: 'light',
  setTheme: (t) => set({ theme: t }),
  // chat
  currentRoomId: undefined,
  setRoom: (id) => set({ currentRoomId: id }),
  unread: {},
  setUnread: (id, count) => set((s) => ({ unread: { ...s.unread, [id]: count } })),
}));
