import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { login } from '../../api-client';

export function LoginPage({ onLogin }: { onLogin: () => void }) {
  const { t } = useTranslation();
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError('');
    try {
      await login(password);
      onLogin();
    } catch {
      setError(t("login.invalidPassword", "Invalid password"));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex items-center justify-center h-screen bg-[var(--color-bg)]">
      <form onSubmit={handleSubmit} className="w-80 p-6 rounded-lg bg-[var(--color-surface)] shadow-lg">
        <h1 className="text-xl font-semibold mb-4 text-[var(--color-text)]">{t("login.title", "Pebble Web")}</h1>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder={t("login.password", "Password")}
          className="w-full px-3 py-2 rounded border border-[var(--color-border)] bg-[var(--color-bg)] text-[var(--color-text)] mb-3"
          autoFocus
        />
        {error && <p className="text-red-500 text-sm mb-2">{error}</p>}
        <button
          type="submit"
          disabled={loading}
          className="w-full py-2 rounded bg-[var(--color-accent)] text-white font-medium"
        >
          {loading ? t("login.loggingIn", "Logging in...") : t("login.submit", "Login")}
        </button>
      </form>
    </div>
  );
}
