import axios from "axios";

const api = axios.create({
  baseURL: "/api/v1",
  headers: { "Content-Type": "application/json" },
});

// Attach auth token to every request
api.interceptors.request.use((config) => {
  const token = localStorage.getItem("pebble_token");
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Redirect to login on 401
api.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      localStorage.removeItem("pebble_token");
      window.location.reload();
    }
    return Promise.reject(error);
  },
);

export default api;

export async function login(password: string): Promise<string> {
  const { data } = await api.post<{ token: string }>("/auth/login", { password });
  localStorage.setItem("pebble_token", data.token);
  return data.token;
}

export function logout(): void {
  localStorage.removeItem("pebble_token");
  window.location.reload();
}

export function isAuthenticated(): boolean {
  return !!localStorage.getItem("pebble_token");
}
