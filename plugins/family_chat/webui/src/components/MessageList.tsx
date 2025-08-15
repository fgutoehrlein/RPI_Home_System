import { FixedSizeList as List } from 'react-window';
import type { FixedSizeList } from 'react-window';
import { useEffect, useRef } from 'react';
import MessageItem from './MessageItem';
import { Message } from '../lib/types';

interface Props {
  messages: Message[];
}

export default function MessageList({ messages }: Props) {
  const itemSize = 80;
  const listRef = useRef<FixedSizeList>(null);

  useEffect(() => {
    listRef.current?.scrollToItem(messages.length - 1);
  }, [messages.length]);

  return (
    <div data-testid="message-list">
      <List
        ref={listRef}
        height={400}
        width={'100%'}
        itemCount={messages.length}
        itemSize={itemSize}
      >
        {({ index, style }) => (
          <div style={style}>
            <MessageItem message={messages[index]} />
          </div>
        )}
      </List>
    </div>
  );
}
