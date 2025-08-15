import { Message } from '../lib/types';
import { renderMarkdown } from '../lib/markdown';

interface Props {
  message: Message;
}

export default function MessageItem({ message }: Props) {
  return (
    <div className="px-4 py-2" data-testid="message-item">
      <div className="text-sm text-gray-600">{message.user.display_name}</div>
      <div
        data-testid="message-text"
        className="prose prose-sm"
        dangerouslySetInnerHTML={{ __html: renderMarkdown(message.text_md) }}
      />
    </div>
  );
}
