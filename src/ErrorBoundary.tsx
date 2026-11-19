import { Component, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

interface State {
  error: string | null;
}

/** Catches a crash in one screen so the app shows a message instead of a blank window. */
export default class ErrorBoundary extends Component<{ children: ReactNode; resetKey?: string }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(e: unknown): State {
    return { error: e instanceof Error ? e.message : String(e) };
  }

  componentDidCatch(e: unknown) {
    const text = e instanceof Error ? `${e.message}\n${e.stack ?? ""}` : String(e);
    invoke("log_frontend_error", { message: text }).catch(() => {});
  }

  componentDidUpdate(prev: { resetKey?: string }) {
    if (this.state.error && prev.resetKey !== this.props.resetKey) this.setState({ error: null });
  }

  render() {
    if (this.state.error) {
      return (
        <div className="section">
          <h2>Something went wrong on this screen</h2>
          <p className="muted">
            The rest of Trayload still works. Try another tab, or press the button. The details were saved to the log
            (Settings, Open log folder).
          </p>
          <p className="error small">{this.state.error}</p>
          <button className="btn" onClick={() => this.setState({ error: null })}>
            Try again
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
