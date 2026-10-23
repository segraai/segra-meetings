"use client";

import { Button } from '@/components/ui/button';
import { Check, Copy, Loader2 } from 'lucide-react';
import Analytics from '@/lib/analytics';

interface SummaryUpdaterButtonGroupProps {
  /** Edits not yet autosaved */
  isDirty: boolean;
  onCopy: () => Promise<void>;
}

/** Autosave status and copy action for the AI notes */
export function SummaryUpdaterButtonGroup({ isDirty, onCopy }: SummaryUpdaterButtonGroupProps) {
  return (
    <div className="flex items-center gap-3">
      <span className="flex items-center gap-1 text-xs text-gray-400" aria-live="polite">
        {isDirty ? (
          <><Loader2 className="h-3 w-3 animate-spin" /> Saving...</>
        ) : (
          <><Check className="h-3 w-3" /> Saved</>
        )}
      </span>
      <Button
        variant="outline"
        size="sm"
        title="Copy Summary"
        onClick={() => {
          Analytics.trackButtonClick('copy_summary', 'meeting_details');
          onCopy();
        }}
        className="cursor-pointer"
      >
        <Copy />
        <span className="hidden @[40rem]:inline">Copy</span>
      </Button>
    </div>
  );
}
