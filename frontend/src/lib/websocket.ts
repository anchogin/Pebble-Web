type EventHandler = (data: any) => void;

class WebSocketClient {
  private ws: WebSocket | null = null;
  private handlers: Map<string, EventHandler[]> = new Map();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private authenticated = false;

  connect() {
    const token = localStorage.getItem("pebble_token");
    if (!token) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/api/v1/ws`;

    this.ws = new WebSocket(url);
    this.authenticated = false;

    this.ws.onopen = () => {
      // Send token as first message for authentication
      this.ws?.send(token);
    };

    this.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (!this.authenticated) {
          if (msg.type === "authenticated") {
            this.authenticated = true;
          } else if (msg.type === "error") {
            this.ws?.close();
          }
          return;
        }
        const handlers = this.handlers.get(msg.type) || [];
        handlers.forEach((h) => h(msg));
        const allHandlers = this.handlers.get("*") || [];
        allHandlers.forEach((h) => h(msg));
      } catch {
        // ignore parse errors
      }
    };

    this.ws.onclose = (event) => {
      const wasAuthenticated = this.authenticated;
      this.authenticated = false;
      // Do not reconnect if the connection was closed due to auth failure
      // (close code 1008 = policy violation, or never authenticated successfully)
      if (wasAuthenticated && event.code !== 1008) {
        this.reconnectTimer = setTimeout(() => this.connect(), 5000);
      }
    };

    this.ws.onerror = () => {
      this.ws?.close();
    };
  }

  disconnect() {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.ws?.close();
    this.ws = null;
    this.authenticated = false;
  }

  on(event: string, handler: EventHandler) {
    const handlers = this.handlers.get(event) || [];
    handlers.push(handler);
    this.handlers.set(event, handlers);
  }

  off(event: string, handler: EventHandler) {
    const handlers = this.handlers.get(event) || [];
    this.handlers.set(
      event,
      handlers.filter((h) => h !== handler),
    );
  }
}

export const wsClient = new WebSocketClient();
