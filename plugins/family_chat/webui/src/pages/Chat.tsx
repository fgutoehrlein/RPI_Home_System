import { useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import Layout from '../components/Layout';
import MessageList from '../components/MessageList';
import Composer from '../components/Composer';
import { Message } from '../lib/types';
import { api } from '../lib/api';
import { connect } from '../lib/ws';
import { getToken } from '../lib/auth';

export default function Chat() {
  const [messages, setMessages] = useState<Message[]>([]);
  const { id } = useParams<{ id: string }>();
  const roomId = id || '1';
  const wsRef = useRef<ReturnType<typeof connect> | null>(null);

  useEffect(() => {
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
  }, [roomId]);

  useEffect(() => {
    const token = getToken();
    if (!token) return;
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
      }
    });
    wsRef.current = ws;
    return () => ws.close();
  }, [roomId]);

  async function send(text: string) {
    const temp: Message = {
      id: `tmp-${Date.now()}`,
      room_id: roomId,
      text_md: text,
      created_at: new Date().toISOString(),
      user: { id: 'me', username: 'me', display_name: 'Me' },
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

  return (
    <Layout>
      <MessageList messages={messages} />
      <Composer onSend={send} />
    </Layout>
  );
}
