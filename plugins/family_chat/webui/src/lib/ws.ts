export type WSEvent =
  | { t: 'presence'; user_id: string; state: string }
  | { t: 'typing'; room_id: string; user_id: string }
  | { t: 'message'; room_id: string; message: any }
  | { t: 'message_edit'; room_id: string; message: any }
  | { t: 'message_delete'; room_id: string; message_id: string }
  | { t: 'read'; room_id: string; user_id: string; message_id: string };

export function connect(token: string, onEvent: (e: WSEvent) => void) {
  const base = (window as any).__FC_BASE__ || import.meta.env.VITE_FAMILY_CHAT_BASE || '';
  const url = base.replace(/^http/, 'ws') + `/ws?token=${token}`;
  let ws: WebSocket | null = null;
  let queue: any[] = [];
  let retry = 1000;

  function send(data: any) {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(data));
    } else {
      queue.push(data);
    }
  }

  function open() {
    ws = new WebSocket(url);
    ws.onopen = () => {
      retry = 1000;
      queue.splice(0).forEach(send);
    };
    ws.onmessage = (ev) => {
      try {
        const data = JSON.parse(ev.data);
        onEvent(data as WSEvent);
      } catch (e) {
        console.error('ws parse', e);
      }
    };
    ws.onclose = () => {
      setTimeout(open, retry);
      retry = Math.min(retry * 2, 10000);
    };
  }

  open();

  return {
    sendTyping(roomId: string) {
      send({ t: 'typing', room_id: roomId });
    },
    sendMessage(payload: any) {
      send({ t: 'send', ...payload });
    },
  };
}
