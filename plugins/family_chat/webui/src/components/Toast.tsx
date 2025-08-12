interface Props {
  message: string;
}

export default function Toast({ message }: Props) {
  return (
    <div className="fixed bottom-4 right-4 rounded bg-gray-800 px-4 py-2 text-white shadow">
      {message}
    </div>
  );
}
