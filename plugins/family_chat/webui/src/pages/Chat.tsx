import { useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import Layout from '../components/Layout';
import MessageList from '../components/MessageList';
import Composer from '../components/Composer';
import TypingIndicator from '../components/TypingIndicator';
import { Message } from '../lib/types';
import { api } from '../lib/api';
import { connect } from '../lib/ws';
import { getToken } from '../lib/auth';
import { useStore } from '../lib/store';

export default function Chat() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [wsConnected, setWsConnected] = useState(false);
  const [typingUsers, setTypingUsers] = useState<Record<string, string>>({});
  const { id } = useParams<{ id: string }>();
  const isValidRoom = id && id.length === 36; // very light UUID check
  const roomId = isValidRoom ? id! : null;
  const wsRef = useRef<ReturnType<typeof connect> | null>(null);
  const typingRef = useRef<Record<string, ReturnType<typeof setTimeout>>>({});
  const me = useStore((s) => s.user);

  useEffect(() => {
    const token = getToken();
    if (!token || !roomId) return;
    const ws = connect(token, (e) => {
      if (e.t === 'message' && e.room_id === roomId) {
        setMessages((m) => {
          const idx = m.findIndex(
            (x) => x.id.startsWith('tmp') && x.text_md === e.message.text_md
          );
          if (idx >= 0) {
            const copy = [...m];
            copy[idx] = e.message;
            return copy;
          }
          if (m.some((x) => x.id === e.message.id)) return m;
          return [...m, e.message];
        });
      } else if (e.t === 'typing' && e.room_id === roomId && e.user_id !== me?.id) {
        setTypingUsers((u) => ({ ...u, [e.user_id]: e.display_name }));
        const t = typingRef.current[e.user_id];
        if (t) clearTimeout(t);
        typingRef.current[e.user_id] = setTimeout(() => {
          setTypingUsers((u) => {
            const { [e.user_id]: _, ...rest } = u;
            return rest;
          });
          delete typingRef.current[e.user_id];
        }, 3000);
      }
    });
    const off = ws.onStatus((s) => setWsConnected(s === 'open'));
    ws.join(roomId);
    wsRef.current = ws;
    api
      .getMessages(roomId)
      .then((msgs) => {
        setMessages((existing) => {
          const ids = new Set(existing.map((m) => m.id));
          const merged = [...existing];
          for (const msg of msgs) {
            if (!ids.has(msg.id)) merged.push(msg);
          }
          return merged;
        });
      })
      .catch(console.error);
    return () => {
      off();
      ws.close();
      Object.values(typingRef.current).forEach(clearTimeout);
      typingRef.current = {};
      setTypingUsers({});
    };
  }, [roomId, me?.id]);

  async function send(text: string) {
    if (!roomId) return;
    const temp: Message = {
      id: `tmp-${Date.now()}`,
      room_id: roomId,
      text_md: text,
      created_at: new Date().toISOString(),
      user: me || { id: 'me', username: 'me', display_name: 'Me' },
    } as Message;
    setMessages((m) => [...m, temp]);
    try {
      const msg = await api.sendMessage({ room_id: roomId, text_md: text });
      setMessages((m) => {
        if (m.some((x) => x.id === msg.id)) {
          return m.filter((x) => x.id !== temp.id);
        }
        return m.map((x) => (x.id === temp.id ? msg : x));
      });
    } catch (e) {
      setMessages((m) => m.filter((x) => x.id !== temp.id));
      console.error(e);
    }
  }

  if (!roomId) {
    return (
      <Layout>
        <div className="p-4 text-gray-500">Select or create a room to start chatting.</div>
      </Layout>
    );
  }

  return (
    <Layout>
      <div data-testid="ws-status" className="hidden">
        {wsConnected ? 'connected' : 'disconnected'}
      </div>
      <MessageList messages={messages} />
      <TypingIndicator names={Object.values(typingUsers)} />
      <Composer
        onSend={send}
        onTyping={() => roomId && wsRef.current?.sendTyping(roomId)}
      />
    </Layout>
  );
}
