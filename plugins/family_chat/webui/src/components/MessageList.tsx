import { FixedSizeList as List, ListOnScrollProps } from 'react-window';
import MessageItem from './MessageItem';
import { Message } from '../lib/types';

interface Props {
  messages: Message[];
}

export default function MessageList({ messages }: Props) {
  const itemSize = 80;
  return (
    <List height={400} width={'100%'} itemCount={messages.length} itemSize={itemSize}>
      {({ index, style }) => (
        <div style={style}>
          <MessageItem message={messages[index]} />
        </div>
      )}
    </List>
  );
}
