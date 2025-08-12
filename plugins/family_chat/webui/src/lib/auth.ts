const TOKEN_KEY = 'fc_token';
let token: string | null = null;

export function getToken(): string | null {
  if (token) return token;
  token = sessionStorage.getItem(TOKEN_KEY);
  return token;
}

export function setToken(t: string) {
  token = t;
  sessionStorage.setItem(TOKEN_KEY, t);
}

export function clearToken() {
  token = null;
  sessionStorage.removeItem(TOKEN_KEY);
}

export function useAuth() {
  return {
    token: getToken(),
    login(t: string) {
      setToken(t);
    },
    logout() {
      clearToken();
    },
  };
}
