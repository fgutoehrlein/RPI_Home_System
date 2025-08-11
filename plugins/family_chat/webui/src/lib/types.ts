export interface User {
  id: string;
  username: string;
  display_name: string;
  presence?: 'online' | 'offline';
}

export interface Room {
  id: string;
  name: string;
  slug?: string;
  unread?: number;
}

export interface Attachment {
  file_id: string;
  name: string;
  size: number;
  mime: string;
}

export interface Message {
  id: string;
  room_id: string;
  user: User;
  text_md: string;
  created_at: string;
  attachments?: Attachment[];
  read_by?: string[];
}

export interface AuthMe {
  user: User;
}

export interface LoginResponse {
  token: string;
  user: User;
}

export interface FileUploadResponse {
  file_id: string;
  name: string;
  size: number;
  sha256: string;
  mime: string;
}

export interface SearchResult {
  message: Message;
  highlights: string[];
}
