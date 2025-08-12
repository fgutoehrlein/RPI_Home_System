import { PropsWithChildren } from 'react';

interface Props extends PropsWithChildren {
  open: boolean;
  onClose: () => void;
}

export default function Modal({ open, onClose, children }: Props) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <div className="bg-white p-4 rounded" onClick={(e) => e.stopPropagation()}>
        {children}
      </div>
    </div>
  );
}
