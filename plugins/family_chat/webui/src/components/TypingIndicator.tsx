import React from 'react';

interface Props {
  names: string[];
}

export default function TypingIndicator({ names }: Props) {
  if (!names.length) return null;
  let text = '';
  if (names.length === 1) {
    text = `${names[0]} is typing...`;
  } else if (names.length === 2) {
    text = `${names[0]} and ${names[1]} are typing...`;
  } else {
    const last = names[names.length - 1];
    text = `${names.slice(0, -1).join(', ')} and ${last} are typing...`;
  }
  return <div className="px-2 py-1 text-xs italic text-gray-500">{text}</div>;
}
