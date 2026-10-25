'use client';

import type { ReactNode } from 'react';
import { FileText, PenLine, Sparkles } from 'lucide-react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

export type MeetingDetailsTab = 'notes' | 'transcript' | 'rawNotes';

const TABS = [
  { value: 'notes' as const, label: 'Notes', icon: Sparkles },
  { value: 'transcript' as const, label: 'Transcript', icon: FileText },
  { value: 'rawNotes' as const, label: 'Raw notes', icon: PenLine },
];

interface MeetingDetailsTabsProps {
  activeTab: MeetingDetailsTab;
  onTabChange: (tab: MeetingDetailsTab) => void;
  content: Record<MeetingDetailsTab, ReactNode>;
}

/** The meeting page's single main pane, with one tab per view of the meeting */
export function MeetingDetailsTabs({ activeTab, onTabChange, content }: MeetingDetailsTabsProps) {
  return (
    <Tabs
      value={activeTab}
      onValueChange={(value) => onTabChange(value as MeetingDetailsTab)}
      className="flex flex-1 min-h-0 min-w-0 flex-col overflow-hidden bg-white"
    >
      <div className="shrink-0 border-b border-gray-200 px-4">
        <TabsList className="h-auto gap-1 rounded-none bg-transparent p-0">
          {TABS.map(tab => {
            const Icon = tab.icon;
            return (
              <TabsTrigger
                key={tab.value}
                value={tab.value}
                className="relative gap-2 rounded-none border-b-2 border-transparent bg-transparent px-3 py-3 text-sm text-gray-600 shadow-none hover:text-gray-900 data-[state=active]:border-blue-600 data-[state=active]:bg-transparent data-[state=active]:text-blue-700 data-[state=active]:shadow-none"
              >
                <Icon className="h-4 w-4" />
                {tab.label}
              </TabsTrigger>
            );
          })}
        </TabsList>
      </div>
      {TABS.map(tab => (
        <TabsContent
          key={tab.value}
          value={tab.value}
          // Kept mounted so playback continues and edits aren't lost while switching tabs
          forceMount
          className="mt-0 flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden data-[state=inactive]:hidden"
        >
          {content[tab.value]}
        </TabsContent>
      ))}
    </Tabs>
  );
}
