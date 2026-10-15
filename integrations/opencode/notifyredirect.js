import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';

// Set NOTIFYREDIRECT_BRIDGE to the absolute Send-AgentEvent.ps1 path.
export const NotifyRedirectPlugin = async ({ directory }) => ({
  event: async ({ event }) => {
    const state = { 'session.idle': 'TurnEnded', 'session.error': 'Failed', 'permission.asked': 'WaitingInput' }[event.type];
    if (!state || !process.env.NOTIFYREDIRECT_BRIDGE) return;
    try {
      const child = spawn('powershell.exe', ['-NoProfile', '-File', process.env.NOTIFYREDIRECT_BRIDGE, '-Agent', 'generic'], { windowsHide: true, stdio: ['pipe', 'ignore', 'ignore'] });
      child.on('error', () => {});
      child.stdin.on('error', () => {});
      child.stdin.end(JSON.stringify({ version: 1, eventId: randomUUID(), source: 'opencode', title: event.type, body: 'OpenCode emitted ' + event.type, state, project: directory, sessionId: event.properties?.sessionID ?? '', occurredAt: new Date().toISOString() }));
      const timeout = setTimeout(() => child.kill(), 5000);timeout.unref();child.once('exit', () => clearTimeout(timeout));
    } catch {}
  }
});
