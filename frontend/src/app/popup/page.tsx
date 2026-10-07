'use client';

import { useEffect, useState } from 'react';
import { CalendarClock, Mic, Video, X } from 'lucide-react';
import { meetingAssistantService } from '@/services/meetingAssistantService';

type PopupKind = 'upcoming' | 'detected' | 'recording';

interface PopupParams {
  kind: PopupKind;
  title: string;
  subtitle: string;
  meetingUrl: string | null;
}

// Unanswered prompts close themselves; "recording started" is just a heads-up
const AUTO_CLOSE_MS: Record<PopupKind, number> = {
  upcoming: 3 * 60_000,
  detected: 2 * 60_000,
  recording: 15_000,
};

export default function MeetingPopup() {
  const [params, setParams] = useState<PopupParams | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const search = new URLSearchParams(window.location.search);
    const kind = (search.get('kind') as PopupKind) || 'upcoming';
    setParams({
      kind,
      title: search.get('title') || 'Meeting',
      subtitle: search.get('subtitle') || '',
      meetingUrl: search.get('meetingUrl'),
    });
    const timer = setTimeout(() => meetingAssistantService.popupAction('dismiss'), AUTO_CLOSE_MS[kind]);
    return () => clearTimeout(timer);
  }, []);

  if (!params) return null;

  const act = async (action: 'record' | 'join' | 'stop' | 'dismiss') => {
    setBusy(true);
    try {
      await meetingAssistantService.popupAction(action, params.title, params.meetingUrl);
    } catch (error) {
      console.error('Popup action failed:', error);
      setBusy(false);
    }
  };

  const Icon = params.kind === 'upcoming' ? CalendarClock : params.kind === 'recording' ? Mic : Video;

  return (
    <div className="h-screen w-screen p-1 select-none">
      <div className="h-full w-full rounded-xl border border-gray-200 bg-white shadow-lg flex flex-col p-4">
        <div className="flex items-start gap-3">
          <div
            className={`mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full ${
              params.kind === 'recording' ? 'bg-red-50 text-red-600' : 'bg-blue-50 text-blue-600'
            }`}
          >
            <Icon className="h-4 w-4" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold text-gray-900" title={params.title}>
              {params.title}
            </p>
            <p className="truncate text-xs text-gray-500">{params.subtitle}</p>
          </div>
          <button
            onClick={() => act('dismiss')}
            className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600"
            aria-label="Dismiss"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-auto flex justify-end gap-2">
          {params.kind === 'recording' ? (
            <>
              <PopupButton variant="secondary" disabled={busy} onClick={() => act('dismiss')}>
                OK
              </PopupButton>
              <PopupButton variant="danger" disabled={busy} onClick={() => act('stop')}>
                Stop recording
              </PopupButton>
            </>
          ) : (
            <>
              <PopupButton variant="secondary" disabled={busy} onClick={() => act('dismiss')}>
                Not now
              </PopupButton>
              {params.kind === 'upcoming' && params.meetingUrl && (
                <PopupButton variant="secondary" disabled={busy} onClick={() => act('join')}>
                  Join &amp; record
                </PopupButton>
              )}
              <PopupButton variant="primary" disabled={busy} onClick={() => act('record')}>
                Record
              </PopupButton>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function PopupButton({
  variant,
  children,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant: 'primary' | 'secondary' | 'danger' }) {
  const styles = {
    primary: 'bg-blue-600 text-white hover:bg-blue-700',
    secondary: 'border border-gray-200 bg-white text-gray-700 hover:bg-gray-50',
    danger: 'bg-red-600 text-white hover:bg-red-700',
  }[variant];
  return (
    <button
      {...props}
      className={`rounded-md px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50 ${styles}`}
    >
      {children}
    </button>
  );
}
