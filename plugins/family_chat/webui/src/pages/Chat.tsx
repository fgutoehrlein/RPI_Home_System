import { useEffect, useState } from 'react';
import Layout from '../components/Layout';
import MessageList from '../components/MessageList';
import Composer from '../components/Composer';
import { Message } from '../lib/types';
import { api } from '../lib/api';

export default function Chat() {
  const [messages, setMessages] = useState<Message[]>([]);

  useEffect(() => {
    api
      .getMessages('1')
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
  }, []);

  async function send(text: string) {
    const temp: Message = {
      id: `tmp-${Date.now()}`,
      room_id: '1',
      text_md: text,
      created_at: new Date().toISOString(),
      user: { id: 'me', username: 'me', display_name: 'Me' },
    } as Message;
    setMessages((m) => [...m, temp]);
    try {
      const msg = await api.sendMessage({ room_id: '1', text_md: text });
      setMessages((m) => m.map((x) => (x.id === temp.id ? msg : x)));
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
