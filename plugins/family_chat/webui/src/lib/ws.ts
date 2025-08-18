export type WSEvent =
  | { t: 'presence'; user_id: string; state: string }
  | { t: 'typing'; room_id: string; user_id: string }
  | { t: 'message'; room_id: string; message: any }
  | { t: 'message_edit'; room_id: string; message: any }
  | { t: 'message_delete'; room_id: string; message_id: string }
  | { t: 'read'; room_id: string; user_id: string; message_id: string };

export function connect(
  token: string,
  onEvent: (e: WSEvent) => void,
) {
  const base =
    (globalThis as any).__FC_BASE__ ||
    (import.meta as any).env?.VITE_FAMILY_CHAT_BASE ||
    '';
  const url = base.replace(/^http/, 'ws') + `/ws?token=${token}`;
  let ws: WebSocket | null = null;
  let queue: any[] = [];
  let retry = 1000;
  let stopped = false;
  const statusListeners = new Set<(status: 'open' | 'closed') => void>();

  function emit(status: 'open' | 'closed') {
    statusListeners.forEach((l) => l(status));
  }

  function send(data: any) {
    if (ws && ws.readyState === globalThis.WebSocket.OPEN) {
      ws.send(JSON.stringify(data));
    } else {
      queue.push(data);
    }
  }

  function open() {
    if (stopped) return;
    ws = new globalThis.WebSocket(url);
    ws.onopen = () => {
      retry = 1000;
      queue.splice(0).forEach(send);
      emit('open');
    };
    ws.onmessage = (ev) => {
      if (typeof ev.data !== 'string') return;
      if (ev.data === 'hello') return;
      try {
        const data = JSON.parse(ev.data);
        onEvent(data as WSEvent);
      } catch (e) {
        console.error('ws parse', e);
      }
    };
    ws.onerror = (err) => {
      console.error('ws error', err);
    };
    ws.onclose = () => {
      emit('closed');
      if (stopped) return;
      setTimeout(open, retry);
      retry = Math.min(retry * 2, 10000);
    };
  }

  open();

  return {
    join(roomId: string) {
      send({ action: 'join', room_id: roomId });
    },
    sendTyping(roomId: string) {
      send({ t: 'typing', room_id: roomId });
    },
    sendMessage(payload: any) {
      send({ t: 'send', ...payload });
    },
    close() {
      stopped = true;
      ws?.close();
    },
    onStatus(fn: (status: 'open' | 'closed') => void) {
      statusListeners.add(fn);
      return () => statusListeners.delete(fn);
    },
  };
}
