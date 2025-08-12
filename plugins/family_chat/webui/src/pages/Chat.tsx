import { useEffect, useState } from 'react';
import Layout from '../components/Layout';
import MessageList from '../components/MessageList';
import Composer from '../components/Composer';
import { Message } from '../lib/types';
import { api } from '../lib/api';

export default function Chat() {
  const [messages, setMessages] = useState<Message[]>([]);

  useEffect(() => {
    api.getMessages('1').then(setMessages).catch(console.error);
  }, []);

  async function send(text: string) {
    const msg = await api.sendMessage({ room_id: '1', text_md: text });
    setMessages((m) => [...m, msg]);
  }

  return (
    <Layout>
      <MessageList messages={messages} />
      <Composer onSend={send} />
    </Layout>
  );
}
