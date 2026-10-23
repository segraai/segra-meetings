'use client';

import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { AlertTriangle, CalendarCheck, ExternalLink, Loader2, RefreshCw } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import { useConfig } from '@/contexts/ConfigContext';
import {
  AssistantSettings,
  AutoRecordMode,
  CalendarStatus,
  meetingAssistantService,
} from '@/services/meetingAssistantService';

const AUTO_RECORD_OPTIONS: { value: AutoRecordMode; label: string; description: string }[] = [
  { value: 'ask', label: 'Ask me', description: 'Show a popup with a Record button when a call starts' },
  { value: 'auto', label: 'Record automatically', description: 'Start recording right away and notify you' },
  { value: 'off', label: 'Off', description: 'Never react to calls' },
];

const GOOGLE_CREDENTIALS_URL = 'https://console.cloud.google.com/apis/credentials';

export function CalendarSettings() {
  const { isAutoSummary, toggleIsAutoSummary } = useConfig();
  const [settings, setSettings] = useState<AssistantSettings | null>(null);
  const [status, setStatus] = useState<CalendarStatus | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);

  useEffect(() => {
    meetingAssistantService.getSettings().then(setSettings).catch(console.error);
    meetingAssistantService.getCalendarStatus().then(setStatus).catch(console.error);
  }, []);

  const update = async (patch: Partial<AssistantSettings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      await meetingAssistantService.saveSettings(next);
      setStatus(await meetingAssistantService.getCalendarStatus());
    } catch (error) {
      toast.error('Failed to save settings', { description: String(error) });
    }
  };

  const connect = async () => {
    setIsConnecting(true);
    toast.info('Finish signing in to Google in your browser');
    try {
      const email = await meetingAssistantService.connectCalendar();
      toast.success(`Connected to ${email}`);
      setStatus(await meetingAssistantService.getCalendarStatus());
    } catch (error) {
      toast.error('Could not connect Google Calendar', { description: String(error) });
    } finally {
      setIsConnecting(false);
    }
  };

  const disconnect = async () => {
    try {
      await meetingAssistantService.disconnectCalendar();
      setStatus(await meetingAssistantService.getCalendarStatus());
      toast.success('Google Calendar disconnected');
    } catch (error) {
      toast.error('Failed to disconnect', { description: String(error) });
    }
  };

  const syncNow = async () => {
    setIsSyncing(true);
    try {
      const count = await meetingAssistantService.syncCalendar();
      toast.success(`Synced ${count} events`);
    } catch (error) {
      toast.error('Calendar sync failed', { description: String(error) });
    } finally {
      setIsSyncing(false);
    }
  };

  if (!settings || !status) {
    return (
      <div className="flex items-center gap-2 p-6 text-sm text-gray-500">
        <Loader2 className="h-4 w-4 animate-spin" /> Loading…
      </div>
    );
  }

  // The user's own OAuth client: required without a built-in client, optional (advanced) with one
  const ownClientSetup = (
    <>
      <details className="rounded-md bg-gray-50 px-4 py-3 text-sm text-gray-600" open={!status.has_credentials}>
        <summary className="cursor-pointer font-medium text-gray-800">One-time setup: create a Google OAuth client</summary>
        <ol className="mt-2 list-decimal space-y-1 pl-5">
          <li>
            In Google Cloud Console, create a project and enable the <strong>Google Calendar API</strong>.
          </li>
          <li>Configure the OAuth consent screen (External, add yourself as a test user).</li>
          <li>
            Under{' '}
            <button
              className="inline-flex items-center gap-0.5 text-blue-600 hover:underline"
              onClick={() => meetingAssistantService.openExternal(GOOGLE_CREDENTIALS_URL)}
            >
              Credentials <ExternalLink className="h-3 w-3" />
            </button>
            , create an <strong>OAuth client ID</strong> of type <strong>Desktop app</strong>.
          </li>
          <li>Paste its client ID and secret below, then click Connect.</li>
        </ol>
      </details>

      <div className="grid gap-3 sm:grid-cols-2">
        <label className="text-sm">
          <span className="mb-1 block font-medium text-gray-700">Client ID</span>
          <input
            value={settings.google_client_id}
            onChange={e => setSettings({ ...settings, google_client_id: e.target.value })}
            onBlur={() => update({})}
            placeholder="xxxx.apps.googleusercontent.com"
            className="w-full rounded-md border border-gray-200 px-3 py-2 text-sm outline-none focus:border-blue-400"
          />
        </label>
        <label className="text-sm">
          <span className="mb-1 block font-medium text-gray-700">Client secret</span>
          <input
            type="password"
            value={settings.google_client_secret}
            onChange={e => setSettings({ ...settings, google_client_secret: e.target.value })}
            onBlur={() => update({})}
            placeholder="GOCSPX-…"
            className="w-full rounded-md border border-gray-200 px-3 py-2 text-sm outline-none focus:border-blue-400"
          />
        </label>
      </div>
    </>
  );

  return (
    <div className="flex flex-col gap-4">
      {/* Google Calendar */}
      <div className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <h3 className="mb-2 text-lg font-semibold text-gray-900">Google Calendar</h3>
        <p className="mb-4 text-sm text-gray-600">
          Shows today&apos;s meetings on the home screen, names recordings after the event, and reminds you
          before meetings start. Read-only access; events are stored only on this computer.
        </p>

        {status.connected && status.needs_reconnect ? (
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-md bg-amber-50 px-4 py-3">
            <div className="flex items-center gap-2 text-sm text-amber-800">
              <AlertTriangle className="h-4 w-4" />
              Google sign-in for <strong>{status.account_email ?? 'your account'}</strong> expired. Calendar sync is paused.
            </div>
            <div className="flex gap-2">
              <button
                onClick={connect}
                disabled={isConnecting}
                className="flex items-center gap-2 rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
              >
                {isConnecting && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
                {isConnecting ? 'Waiting for Google sign-in…' : 'Reconnect'}
              </button>
              <button
                onClick={disconnect}
                className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-red-600 hover:bg-red-50"
              >
                Disconnect
              </button>
            </div>
          </div>
        ) : status.connected ? (
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-md bg-green-50 px-4 py-3">
            <div className="flex items-center gap-2 text-sm text-green-800">
              <CalendarCheck className="h-4 w-4" />
              Connected as <strong>{status.account_email ?? 'Google account'}</strong>
            </div>
            <div className="flex gap-2">
              <button
                onClick={syncNow}
                disabled={isSyncing}
                className="flex items-center gap-1 rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50 disabled:opacity-50"
              >
                <RefreshCw className={`h-3.5 w-3.5 ${isSyncing ? 'animate-spin' : ''}`} /> Sync now
              </button>
              <button
                onClick={disconnect}
                className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-red-600 hover:bg-red-50"
              >
                Disconnect
              </button>
            </div>
          </div>
        ) : (
          <div className="space-y-4">
            {status.has_builtin_client ? (
              <details className="text-sm text-gray-600">
                <summary className="cursor-pointer text-gray-500 hover:text-gray-800">
                  Advanced: use your own Google OAuth client
                </summary>
                <div className="mt-3 space-y-4">{ownClientSetup}</div>
              </details>
            ) : ownClientSetup}

            <button
              onClick={async () => {
                await update({});
                await connect();
              }}
              disabled={isConnecting || (!status.has_builtin_client && (!settings.google_client_id.trim() || !settings.google_client_secret.trim()))}
              className="flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
            >
              {isConnecting && <Loader2 className="h-4 w-4 animate-spin" />}
              {isConnecting ? 'Waiting for Google sign-in…' : 'Connect Google Calendar'}
            </button>
          </div>
        )}
      </div>

      {/* Reminders */}
      <div className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h3 className="mb-2 text-lg font-semibold text-gray-900">Meeting reminders</h3>
            <p className="text-sm text-gray-600">Pop up a reminder with Join and Record buttons before calendar events.</p>
          </div>
          <Switch
            checked={settings.meeting_reminders_enabled}
            onCheckedChange={checked => update({ meeting_reminders_enabled: checked })}
          />
        </div>
        {settings.meeting_reminders_enabled && (
          <label className="mt-4 flex items-center gap-2 text-sm text-gray-700">
            Remind me
            <select
              value={settings.reminder_minutes_before}
              onChange={e => update({ reminder_minutes_before: Number(e.target.value) })}
              className="rounded-md border border-gray-200 px-2 py-1 text-sm"
            >
              {[0, 1, 2, 5, 10].map(minutes => (
                <option key={minutes} value={minutes}>
                  {minutes === 0 ? 'when it starts' : `${minutes} min before`}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      {/* Call detection */}
      <div className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
        <h3 className="mb-2 text-lg font-semibold text-gray-900">When a call starts</h3>
        <p className="mb-4 text-sm text-gray-600">
          Segra Meetings notices when Zoom, Teams, Slack or a browser (Google Meet) starts using your microphone.
        </p>
        <div className="space-y-2">
          {AUTO_RECORD_OPTIONS.map(option => (
            <label
              key={option.value}
              className={`flex cursor-pointer items-start gap-3 rounded-md border px-4 py-3 ${
                settings.auto_record_mode === option.value ? 'border-blue-400 bg-blue-50' : 'border-gray-200'
              }`}
            >
              <input
                type="radio"
                name="auto-record-mode"
                className="mt-1"
                checked={settings.auto_record_mode === option.value}
                onChange={() => update({ auto_record_mode: option.value })}
              />
              <span>
                <span className="block text-sm font-medium text-gray-900">{option.label}</span>
                <span className="block text-xs text-gray-500">{option.description}</span>
              </span>
            </label>
          ))}
        </div>
        {settings.auto_record_mode === 'auto' && (
          <p className="mt-3 text-xs text-gray-500">
            Browser calls are only recorded automatically when a calendar event with a meeting link is happening;
            otherwise you&apos;ll be asked.
          </p>
        )}

        <div className="mt-5 flex items-center justify-between gap-4 border-t border-gray-100 pt-4">
          <div>
            <p className="text-sm font-medium text-gray-900">Stop recording when the call ends</p>
            <p className="text-xs text-gray-500">Stops ~20 seconds after the meeting app releases the microphone.</p>
          </div>
          <Switch
            checked={settings.auto_stop_when_call_ends}
            onCheckedChange={checked => update({ auto_stop_when_call_ends: checked })}
          />
        </div>
        <div className="mt-4 flex items-center justify-between gap-4 border-t border-gray-100 pt-4">
          <div>
            <p className="text-sm font-medium text-gray-900">Generate notes when recording stops</p>
            <p className="text-xs text-gray-500">Uses your summary model from the Summary tab.</p>
          </div>
          <Switch checked={isAutoSummary} onCheckedChange={toggleIsAutoSummary} />
        </div>
      </div>
    </div>
  );
}
