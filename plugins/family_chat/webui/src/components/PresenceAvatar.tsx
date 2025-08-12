interface Props {
  name: string;
  online?: boolean;
}

export default function PresenceAvatar({ name, online }: Props) {
  return (
    <div className="relative inline-block">
      <div className="h-8 w-8 rounded-full bg-gray-300 flex items-center justify-center">
        {name[0]}
      </div>
      <span
        className={`absolute bottom-0 right-0 block h-2 w-2 rounded-full ${online ? 'bg-green-500' : 'bg-gray-400'}`}
      />
    </div>
  );
}
