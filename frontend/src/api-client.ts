import axios from 'axios';

const api = axios.create({
  baseURL: '/api/v1',
});

api.interceptors.request.use((config) => {
  const token = localStorage.getItem('pebble_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

api.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      localStorage.removeItem('pebble_token');
      window.location.href = '/login';
    }
    return Promise.reject(error);
  }
);

export default api;

export async function login(password: string): Promise<string> {
  const { data } = await api.post('/auth/login', { password });
  localStorage.setItem('pebble_token', data.token);
  return data.token;
}

export function logout(): void {
  localStorage.removeItem('pebble_token');
  window.location.href = '/login';
}

export function isAuthenticated(): boolean {
  return !!localStorage.getItem('pebble_token');
}
