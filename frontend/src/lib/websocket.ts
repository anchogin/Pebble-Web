type EventHandler = (data: any) => void;

class WebSocketClient {
  private ws: WebSocket | null = null;
  private handlers: Map<string, EventHandler[]> = new Map();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private authenticated = false;

  connect() {
    if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) {
      console.log("[WS] already connected/connecting, skipping connect()");
      return;
    }

    const token = localStorage.getItem("pebble_token");
    if (!token) {
      console.warn("[WS] no pebble_token in localStorage, skipping connect");
      return;
    }

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/api/v1/ws`;
    console.log("[WS] connecting to", url, "authenticated=", this.authenticated);

    this.ws = new WebSocket(url);
    this.authenticated = false;

    this.ws.onopen = () => {
      console.log("[WS] connection opened, sending token");
      // Send token as first message for authentication
      this.ws?.send(token);
    };

    this.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (!this.authenticated) {
          if (msg.type === "authenticated") {
            this.authenticated = true;
            console.log("[WS] authenticated ✓");
          } else if (msg.type === "error") {
            console.warn("[WS] auth error, closing:", msg);
            this.ws?.close();
          }
          return;
        }
        if (msg.type?.startsWith("sync")) {
          console.log("[WS] sync msg:", msg.type, msg);
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
      console.log("[WS] closed, code=", event.code, "wasAuth=", this.authenticated);
      const wasAuthenticated = this.authenticated;
      this.authenticated = false;
      // Do not reconnect on auth failures (1008 = policy violation, 4001 = our custom unauthorized)
      const isAuthFailure = event.code === 1008 || event.code === 4001;
      if (wasAuthenticated && !isAuthFailure) {
        console.log("[WS] scheduling reconnect in 5s");
        this.reconnectTimer = setTimeout(() => this.connect(), 5000);
      }
    };

    this.ws.onerror = (err) => {
      console.error("[WS] error:", err);
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
