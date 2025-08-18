const TOKEN_KEY = 'fc_token';

export function getToken(): string | null {
  return sessionStorage.getItem(TOKEN_KEY);
}

export function setToken(t: string) {
  sessionStorage.setItem(TOKEN_KEY, t);
}

export function clearToken() {
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
