'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import { listen } from '@tauri-apps/api/event';
import { AlertTriangle, CalendarDays, FileText, Loader2, Mic, Search, Users, Video, X } from 'lucide-react';
import {
  CALENDAR_UPDATED_EVENT,
  CalendarEvent,
  MeetingOverview,
  MeetingSearchHit,
  meetingAssistantService,
} from '@/services/meetingAssistantService';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { MeetingsChat } from './MeetingsChat';

const PAST_PAGE_SIZE = 25;
const UPCOMING_COUNT = 5;
const UPCOMING_DAYS = 7;
// A recording counts as an event's notes if it started within this window around the event
const EVENT_MATCH_LEAD_MS = 15 * 60_000;

interface UpcomingItem {
  event: CalendarEvent;
  /** Recording already made for this event (when it is happening now) */
  meeting?: MeetingOverview;
  start: Date;
  end: Date;
}

function startOfDay(date: Date) {
  const d = new Date(date);
  d.setHours(0, 0, 0, 0);
  return d;
}

function formatTime(date: Date) {
  return date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
}

function formatDuration(seconds: number | null) {
  if (!seconds || seconds < 60) return null;
  const minutes = Math.round(seconds / 60);
  return minutes >= 60 ? `${Math.floor(minutes / 60)}h ${minutes % 60}m` : `${minutes} min`;
}

function dayLabel(date: Date, today: Date) {
  const days = Math.round((startOfDay(today).getTime() - startOfDay(date).getTime()) / 86_400_000);
  if (days === 0) return 'Today';
  if (days === 1) return 'Yesterday';
  if (days === -1) return 'Tomorrow';
  if (Math.abs(days) < 7) return date.toLocaleDateString(undefined, { weekday: 'long' });
  return date.toLocaleDateString(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    year: date.getFullYear() === today.getFullYear() ? undefined : 'numeric',
  });
}

/** Strips Markdown to a single readable line */
function plainPreview(markdown: string | null) {
  if (!markdown) return null;
  const line = markdown
    .split('\n')
    .map(l => l.replace(/^[#>*\-\s]+/, '').replace(/\*\*/g, '').trim())
    .filter(l => l && !/^summary$/i.test(l))
    .join(' · ');
  return line || null;
}

export function HomeDashboard() {
  const router = useRouter();
  const { setCurrentMeeting } = useSidebar();
  const [meetings, setMeetings] = useState<MeetingOverview[]>([]);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [calendarConnected, setCalendarConnected] = useState<boolean | null>(null);
  const [calendarNeedsReconnect, setCalendarNeedsReconnect] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [pastLimit, setPastLimit] = useState(PAST_PAGE_SIZE);
  const [now, setNow] = useState(() => new Date());

  const [query, setQuery] = useState('');
  const [results, setResults] = useState<MeetingSearchHit[] | null>(null);
  const [isSearching, setIsSearching] = useState(false);

  const loadEvents = useCallback(async () => {
    try {
      const status = await meetingAssistantService.getCalendarStatus();
      setCalendarConnected(status.connected);
      setCalendarNeedsReconnect(status.connected && status.needs_reconnect);
      if (!status.connected) {
        setEvents([]);
        return;
      }
      const from = new Date();
      const to = new Date(from.getTime() + UPCOMING_DAYS * 86_400_000);
      setEvents(await meetingAssistantService.listEvents(from, to));
    } catch (error) {
      console.error('Failed to load calendar events:', error);
    }
  }, []);

  useEffect(() => {
    meetingAssistantService
      .listMeetings()
      .then(setMeetings)
      .catch(error => console.error('Failed to load meetings:', error))
      .finally(() => setIsLoading(false));
    loadEvents();

    const unlisten = listen(CALENDAR_UPDATED_EVENT, () => loadEvents());
    // Keep "now"/"upcoming" labels fresh
    const timer = setInterval(() => setNow(new Date()), 60_000);
    return () => {
      unlisten.then(fn => fn());
      clearInterval(timer);
    };
  }, [loadEvents]);

  // Debounced search
  useEffect(() => {
    const trimmed = query.trim();
    if (!trimmed) {
      setResults(null);
      return;
    }
    setIsSearching(true);
    const timer = setTimeout(async () => {
      try {
        setResults(await meetingAssistantService.searchMeetings(trimmed));
      } catch (error) {
        console.error('Search failed:', error);
        setResults([]);
      } finally {
        setIsSearching(false);
      }
    }, 250);
    return () => clearTimeout(timer);
  }, [query]);

  const openMeeting = (id: string, title: string) => {
    setCurrentMeeting({ id, title });
    router.push(`/meeting-details?id=${id}`);
  };

  const { upcomingGroups, pastGroups, pastCount } = useMemo(() => {
    const upcoming: UpcomingItem[] = events
      .filter(e => !e.is_all_day && new Date(e.end_time) > now)
      .sort((a, b) => a.start_time.localeCompare(b.start_time))
      .slice(0, UPCOMING_COUNT)
      .map(event => {
        const start = new Date(event.start_time);
        const end = new Date(event.end_time);
        const meeting = meetings.find(m => {
          const created = new Date(m.created_at).getTime();
          return created >= start.getTime() - EVENT_MATCH_LEAD_MS && created < end.getTime();
        });
        return { event, meeting, start, end };
      });

    const groupByDay = <T,>(items: T[], dateOf: (item: T) => Date) => {
      const groups: { label: string; items: T[] }[] = [];
      for (const item of items) {
        const label = dayLabel(dateOf(item), now);
        const last = groups[groups.length - 1];
        if (last?.label === label) last.items.push(item);
        else groups.push({ label, items: [item] });
      }
      return groups;
    };

    return {
      upcomingGroups: groupByDay(upcoming, item => item.start),
      pastGroups: groupByDay(meetings.slice(0, pastLimit), m => new Date(m.created_at)),
      pastCount: meetings.length,
    };
  }, [meetings, events, now, pastLimit]);

  return (
    <div className="h-full w-full overflow-y-auto bg-white">
      <div className="mx-auto grid max-w-6xl gap-6 px-6 pb-36 pt-8 xl:grid-cols-[minmax(0,1fr)_360px]">
        <div className="min-w-0 space-y-8">
          {/* Search */}
          <div className="relative">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" />
            <input
              value={query}
              onChange={e => setQuery(e.target.value)}
              placeholder="Search meetings, notes and transcripts…"
              className="w-full rounded-lg border border-gray-200 bg-gray-50 py-2.5 pl-9 pr-9 text-sm outline-none focus:border-blue-400 focus:bg-white"
            />
            {query && (
              <button
                onClick={() => setQuery('')}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
                aria-label="Clear search"
              >
                <X className="h-4 w-4" />
              </button>
            )}
          </div>

          {results !== null || isSearching ? (
            <section>
              <SectionHeading>
                {isSearching ? 'Searching…' : `${results?.length ?? 0} result${results?.length === 1 ? '' : 's'}`}
              </SectionHeading>
              <div className="divide-y divide-gray-100">
                {results?.map(hit => (
                  <button
                    key={hit.id}
                    onClick={() => openMeeting(hit.id, hit.title)}
                    className="block w-full px-2 py-3 text-left hover:bg-gray-50"
                  >
                    <div className="flex items-baseline justify-between gap-3">
                      <span className="truncate text-sm font-medium text-gray-900">{hit.title}</span>
                      <span className="shrink-0 text-xs text-gray-400">
                        {new Date(hit.created_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}
                      </span>
                    </div>
                    {hit.snippet && (
                      <p className="mt-0.5 line-clamp-2 text-xs text-gray-500">
                        {hit.matched_in !== 'title' && (
                          <span className="mr-1 rounded bg-gray-100 px-1 text-[10px] uppercase text-gray-500">
                            {hit.matched_in}
                          </span>
                        )}
                        {hit.snippet}
                      </p>
                    )}
                  </button>
                ))}
              </div>
            </section>
          ) : (
            <>
              {/* Upcoming */}
              <section>
                <SectionHeading>Upcoming</SectionHeading>
                {calendarNeedsReconnect && (
                  <div className="mb-2 flex items-center gap-2 rounded-lg bg-amber-50 px-3 py-2 text-sm text-amber-800">
                    <AlertTriangle className="h-4 w-4 shrink-0" />
                    <span>Google Calendar sign-in expired; events may be out of date.</span>
                    <button
                      className="ml-auto shrink-0 font-medium text-blue-600 hover:underline"
                      onClick={() => router.push('/settings?tab=calendar')}
                    >
                      Reconnect
                    </button>
                  </div>
                )}
                {upcomingGroups.length === 0 ? (
                  <EmptyState>
                    {calendarConnected === false ? (
                      <>
                        <button className="text-blue-600 hover:underline" onClick={() => router.push('/settings?tab=calendar')}>
                          Connect Google Calendar
                        </button>{' '}
                        to see your upcoming meetings.
                      </>
                    ) : (
                      `No meetings in the next ${UPCOMING_DAYS} days.`
                    )}
                  </EmptyState>
                ) : (
                  <div className="space-y-4">
                    {upcomingGroups.map(group => (
                      <div key={group.label}>
                        <p className="mb-1 px-2 text-xs font-medium text-gray-400">{group.label}</p>
                        <div className="space-y-1">
                          {group.items.map(item => (
                            <EventRow
                              key={item.event.id}
                              item={item}
                              now={now}
                              onOpen={() => item.meeting && openMeeting(item.meeting.id, item.meeting.title)}
                            />
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </section>

              {/* Past */}
              <section>
                <SectionHeading>Past meetings</SectionHeading>
                {isLoading ? (
                  <div className="flex items-center gap-2 px-2 text-sm text-gray-400">
                    <Loader2 className="h-4 w-4 animate-spin" /> Loading…
                  </div>
                ) : pastCount === 0 ? (
                  <EmptyState>Your recorded meetings will appear here.</EmptyState>
                ) : (
                  <div className="space-y-5">
                    {pastGroups.map(group => (
                      <div key={group.label}>
                        <p className="mb-1 px-2 text-xs font-medium text-gray-400">{group.label}</p>
                        <div className="space-y-1">
                          {group.items.map(meeting => (
                            <MeetingRow
                              key={meeting.id}
                              meeting={meeting}
                              onOpen={() => openMeeting(meeting.id, meeting.title)}
                            />
                          ))}
                        </div>
                      </div>
                    ))}
                    {pastCount > pastLimit && (
                      <button
                        onClick={() => setPastLimit(limit => limit + PAST_PAGE_SIZE)}
                        className="px-2 text-sm text-blue-600 hover:underline"
                      >
                        Show more ({pastCount - pastLimit} older)
                      </button>
                    )}
                  </div>
                )}
              </section>
            </>
          )}
        </div>

        {/* Chat */}
        <aside className="xl:sticky xl:top-8 xl:h-[calc(100vh-12rem)]">
          <MeetingsChat />
        </aside>
      </div>
    </div>
  );
}

function SectionHeading({ children }: { children: React.ReactNode }) {
  return <h2 className="mb-2 px-2 text-sm font-semibold text-gray-900">{children}</h2>;
}

function EmptyState({ children }: { children: React.ReactNode }) {
  return <p className="rounded-lg border border-dashed border-gray-200 px-4 py-6 text-center text-sm text-gray-500">{children}</p>;
}

function MeetingRow({ meeting, onOpen }: { meeting: MeetingOverview; onOpen: () => void }) {
  const preview = plainPreview(meeting.summary_preview);
  const duration = formatDuration(meeting.duration_seconds);
  return (
    <button onClick={onOpen} className="flex w-full items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-gray-50">
      <FileText className="mt-0.5 h-4 w-4 shrink-0 text-gray-400" />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-3">
          <span className="truncate text-sm font-medium text-gray-900">{meeting.title}</span>
          <span className="shrink-0 text-xs text-gray-400">
            {formatTime(new Date(meeting.created_at))}
            {duration && ` · ${duration}`}
          </span>
        </div>
        <p className="truncate text-xs text-gray-500">{preview ?? 'No AI notes yet'}</p>
      </div>
    </button>
  );
}

function EventRow({
  item,
  now,
  onOpen,
}: {
  item: UpcomingItem;
  now: Date;
  onOpen: () => void;
}) {
  const { event, meeting, start, end } = item;
  const isLive = start <= now && now < end;
  const isPast = end <= now;
  const attendees: { email?: string; name?: string }[] = (() => {
    try {
      return event.attendees_json ? JSON.parse(event.attendees_json) : [];
    } catch {
      return [];
    }
  })();

  return (
    <div
      onClick={meeting ? onOpen : undefined}
      className={`flex items-center gap-3 rounded-lg px-2 py-2 ${meeting ? 'cursor-pointer hover:bg-gray-50' : ''}`}
    >
      <div className="w-16 shrink-0 text-xs text-gray-500">
        <div className={isLive ? 'font-semibold text-blue-600' : ''}>{formatTime(start)}</div>
        <div className="text-gray-400">{formatTime(end)}</div>
      </div>
      <div className={`h-9 w-1 shrink-0 rounded-full ${isLive ? 'bg-blue-500' : isPast ? 'bg-gray-200' : 'bg-blue-200'}`} />
      <div className="min-w-0 flex-1">
        <p className={`truncate text-sm font-medium ${isPast && !meeting ? 'text-gray-400' : 'text-gray-900'}`}>
          {event.title}
        </p>
        <p className="flex items-center gap-2 truncate text-xs text-gray-500">
          {attendees.length > 0 && (
            <span className="flex items-center gap-1" title={attendees.map(a => a.name || a.email).join(', ')}>
              <Users className="h-3 w-3" /> {attendees.length}
            </span>
          )}
          {meeting ? (
            <span className="flex items-center gap-1 text-green-600">
              <FileText className="h-3 w-3" /> Notes ready
            </span>
          ) : isLive ? (
            <span className="text-blue-600">Happening now</span>
          ) : isPast ? (
            <span>Not recorded</span>
          ) : null}
        </p>
      </div>
      {!meeting && !isPast && (
        <div className="flex shrink-0 gap-1.5">
          {event.meeting_url && (
            <button
              onClick={() => meetingAssistantService.popupAction('join', event.title, event.meeting_url)}
              className="flex items-center gap-1 rounded-md border border-gray-200 px-2 py-1 text-xs text-gray-700 hover:bg-gray-50"
              title="Open the meeting link and start recording"
            >
              <Video className="h-3 w-3" /> Join
            </button>
          )}
          <button
            onClick={() => meetingAssistantService.popupAction('record', event.title)}
            className="flex items-center gap-1 rounded-md bg-blue-600 px-2 py-1 text-xs text-white hover:bg-blue-700"
          >
            <Mic className="h-3 w-3" /> Record
          </button>
        </div>
      )}
      {!meeting && isPast && <CalendarDays className="h-4 w-4 shrink-0 text-gray-300" />}
    </div>
  );
}
