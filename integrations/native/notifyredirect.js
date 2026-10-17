import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';

// Set NOTIFYREDIRECT_NATIVE_BRIDGE to the absolute Send-AgentEvent.ps1 path.
export const NotifyRedirectPlugin = async ({ directory }) => ({
  event: async ({ event }) => {
    const state = { 'session.idle': 'turn_ended', 'session.error': 'failed', 'permission.asked': 'waiting_input' }[event.type];
    if (!state || !process.env.NOTIFYREDIRECT_NATIVE_BRIDGE) return;
    try {
      const child = spawn('powershell.exe', ['-NoProfile', '-File', process.env.NOTIFYREDIRECT_NATIVE_BRIDGE, '-Agent', 'generic'], { windowsHide: true, stdio: ['pipe', 'ignore', 'ignore'] });
      child.on('error', () => {});
      child.stdin.on('error', () => {});
      child.stdin.end(JSON.stringify({ version: 1, event_id: randomUUID(), source: 'opencode', title: event.type, body: 'OpenCode emitted ' + event.type, state, project: directory, session_id: event.properties?.sessionID ?? '', occurred_at: new Date().toISOString() }));
      const timeout = setTimeout(() => child.kill(), 5000);timeout.unref();child.once('exit', () => clearTimeout(timeout));
    } catch {}
  }
});
