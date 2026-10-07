'use client';

import { useEffect, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { ArrowUp, Loader2, MessageSquare, RotateCcw } from 'lucide-react';
import { ChatTurn, meetingAssistantService } from '@/services/meetingAssistantService';

interface ChatMessage extends ChatTurn {
  sources?: { id: string; title: string; created_at: string }[];
  isError?: boolean;
}

const SUGGESTIONS = [
  'What action items came out of my meetings this week?',
  'What did we decide yesterday?',
  'Summarize my last meeting',
];

export function MeetingsChat() {
  const router = useRouter();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [isThinking, setIsThinking] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
  }, [messages, isThinking]);

  const ask = async (question: string) => {
    const trimmed = question.trim();
    if (!trimmed || isThinking) return;

    const history: ChatTurn[] = messages
      .filter(m => !m.isError)
      .map(({ role, content }) => ({ role, content }));
    setMessages(prev => [...prev, { role: 'user', content: trimmed }]);
    setInput('');
    setIsThinking(true);
    try {
      const result = await meetingAssistantService.askMeetings(trimmed, history);
      setMessages(prev => [...prev, { role: 'assistant', content: result.answer, sources: result.sources }]);
    } catch (error) {
      setMessages(prev => [
        ...prev,
        { role: 'assistant', content: `Couldn't answer: ${String(error)}`, isError: true },
      ]);
    } finally {
      setIsThinking(false);
    }
  };

  return (
    <div className="flex h-full min-h-[320px] flex-col rounded-xl border border-gray-200 bg-white">
      <div className="flex items-center justify-between border-b border-gray-100 px-4 py-3">
        <div className="flex items-center gap-2 text-sm font-semibold text-gray-800">
          <MessageSquare className="h-4 w-4 text-blue-600" />
          Ask your meetings
        </div>
        {messages.length > 0 && (
          <button
            onClick={() => setMessages([])}
            className="flex items-center gap-1 text-xs text-gray-400 hover:text-gray-600"
            title="New conversation"
          >
            <RotateCcw className="h-3 w-3" /> New
          </button>
        )}
      </div>

      <div ref={scrollRef} className="flex-1 space-y-4 overflow-y-auto px-4 py-3">
        {messages.length === 0 && (
          <div className="space-y-2">
            <p className="text-xs text-gray-500">
              Ask anything about your past meetings. Answers use your notes and transcripts with your
              configured AI model.
            </p>
            {SUGGESTIONS.map(suggestion => (
              <button
                key={suggestion}
                onClick={() => ask(suggestion)}
                className="block w-full rounded-lg border border-gray-100 bg-gray-50 px-3 py-2 text-left text-xs text-gray-600 hover:border-gray-200 hover:bg-gray-100"
              >
                {suggestion}
              </button>
            ))}
          </div>
        )}

        {messages.map((message, index) =>
          message.role === 'user' ? (
            <div key={index} className="flex justify-end">
              <div className="max-w-[85%] rounded-2xl rounded-br-sm bg-blue-600 px-3 py-2 text-sm text-white">
                {message.content}
              </div>
            </div>
          ) : (
            <div key={index} className="text-sm text-gray-800">
              <div
                className={`prose prose-sm max-w-none prose-p:my-1.5 prose-ul:my-1.5 prose-li:my-0.5 ${
                  message.isError ? 'text-red-600' : ''
                }`}
              >
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.content}</ReactMarkdown>
              </div>
              {message.sources && message.sources.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {message.sources.map(source => (
                    <button
                      key={source.id}
                      onClick={() => router.push(`/meeting-details?id=${source.id}`)}
                      className="max-w-full truncate rounded-full border border-gray-200 px-2 py-0.5 text-[11px] text-gray-500 hover:border-blue-300 hover:text-blue-600"
                      title={source.title}
                    >
                      {source.title} ·{' '}
                      {new Date(source.created_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )
        )}

        {isThinking && (
          <div className="flex items-center gap-2 text-xs text-gray-400">
            <Loader2 className="h-3.5 w-3.5 animate-spin" /> Reading your meetings…
          </div>
        )}
      </div>

      <form
        onSubmit={e => {
          e.preventDefault();
          ask(input);
        }}
        className="border-t border-gray-100 p-3"
      >
        <div className="flex items-end gap-2 rounded-lg border border-gray-200 px-3 py-2 focus-within:border-blue-400">
          <textarea
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                ask(input);
              }
            }}
            rows={1}
            placeholder="Ask a question…"
            className="max-h-32 flex-1 resize-none bg-transparent text-sm outline-none placeholder:text-gray-400"
          />
          <button
            type="submit"
            disabled={!input.trim() || isThinking}
            className="flex h-7 w-7 items-center justify-center rounded-full bg-blue-600 text-white disabled:bg-gray-200"
            aria-label="Send"
          >
            <ArrowUp className="h-4 w-4" />
          </button>
        </div>
      </form>
    </div>
  );
}
