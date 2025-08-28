import { Message } from '../lib/types';
import { renderMarkdown } from '../lib/markdown';

interface Props {
  message: Message;
}

export default function MessageItem({ message }: Props) {
  return (
    <div className="px-4 py-2">
      <div className="text-sm text-gray-600">
        {message.user?.display_name}
        {message.user?.username && (
          <span className="ml-1 text-xs text-gray-500">@{message.user.username}</span>
        )}
      </div>
      <div
        className="prose prose-sm"
        dangerouslySetInnerHTML={{ __html: renderMarkdown(message.text_md) }}
        data-testid="message-text"
      />
    </div>
  );
}
