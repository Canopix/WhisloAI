export {};

declare global {
  interface Window {
    __TAURI__?: {
      core?: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
      event?: { listen: (event: string, handler: (e: { payload: unknown }) => void) => Promise<() => void> };
    };
  }
}
