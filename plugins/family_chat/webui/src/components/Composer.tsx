import { useState, KeyboardEvent } from 'react';

interface Props {
  onSend: (text: string) => void;
  onTyping?: () => void;
}

export default function Composer({ onSend, onTyping }: Props) {
  const [text, setText] = useState('');

  function handleKey(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (text.trim()) {
        onSend(text);
        setText('');
      }
    }
  }

  return (
    <div className="border-t p-2">
      <textarea
        className="w-full resize-none rounded border p-2"
        placeholder="Type a message"
        value={text}
        rows={3}
        onKeyDown={handleKey}
        onChange={(e) => {
          setText(e.target.value);
          onTyping?.();
        }}
        data-testid="composer-input"
      />
    </div>
  );
}
