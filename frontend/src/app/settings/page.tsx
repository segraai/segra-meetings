'use client';

import React, { useState, useEffect } from 'react';
import { ArrowLeft, Settings2, Mic, Database as DatabaseIcon, SparkleIcon, FlaskConical, CalendarDays, Info } from 'lucide-react';
import { useRouter } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { TranscriptSettings } from '@/components/TranscriptSettings';
import { RecordingSettings } from '@/components/RecordingSettings';
import { PreferenceSettings } from '@/components/PreferenceSettings';
import { SummaryModelSettings } from '@/components/SummaryModelSettings';
import { BetaSettings } from '@/components/BetaSettings';
import { CalendarSettings } from '@/components/CalendarSettings';
import { About } from '@/components/About';
import { useConfig } from '@/contexts/ConfigContext';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';

// Tabs configuration (constant)
const TABS = [
  { value: 'general', label: 'General', icon: Settings2 },
  { value: 'calendar', label: 'Calendar & Meetings', icon: CalendarDays },
  { value: 'recording', label: 'Recordings', icon: Mic },
  { value: 'Transcriptionmodels', label: 'Transcription', icon: DatabaseIcon },
  { value: 'summaryModels', label: 'Summary', icon: SparkleIcon },
  { value: 'beta', label: 'Beta', icon: FlaskConical },
  { value: 'about', label: 'About', icon: Info }
] as const;

export default function SettingsPage() {
  const router = useRouter();
  const { transcriptModelConfig, setTranscriptModelConfig } = useConfig();

  const [activeTab, setActiveTab] = useState('general');

  // Allow deep links like /settings?tab=calendar
  useEffect(() => {
    const tab = new URLSearchParams(window.location.search).get('tab');
    if (tab && TABS.some(t => t.value === tab)) {
      setActiveTab(tab);
    }
  }, []);

  // Load saved transcript configuration on mount
  useEffect(() => {
    const loadTranscriptConfig = async () => {
      try {
        const config = await invoke('api_get_transcript_config') as any;
        if (config) {
          console.log('Loaded saved transcript config:', config);
          setTranscriptModelConfig({
            provider: config.provider || 'localWhisper',
            model: config.model || 'large-v3',
            apiKey: config.apiKey || null
          });
        }
      } catch (error) {
        console.error('Failed to load transcript config:', error);
      }
    };
    loadTranscriptConfig();
  }, [setTranscriptModelConfig]);

  // Back to where Settings was opened from, or Home when there's no history (e.g. a deep link)
  const goBack = () => {
    if (window.history.length > 1) router.back();
    else router.push('/');
  };

  return (
    <Tabs
      value={activeTab}
      onValueChange={setActiveTab}
      orientation="vertical"
      className="h-screen flex bg-gray-50"
    >
      {/* Left navigation */}
      <aside className="w-56 shrink-0 flex flex-col border-r border-gray-200 bg-white">
        <div className="px-4 pt-5 pb-3">
          <button
            onClick={goBack}
            className="flex items-center gap-1.5 text-sm text-gray-600 hover:text-gray-900 transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            <span>Back</span>
          </button>
          <h1 className="mt-3 text-2xl font-bold">Settings</h1>
        </div>
        <TabsList className="flex h-auto flex-col items-stretch justify-start gap-0.5 rounded-none bg-transparent px-2 py-1">
          {TABS.map(tab => {
            const Icon = tab.icon;
            return (
              <TabsTrigger
                key={tab.value}
                value={tab.value}
                className="justify-start gap-3 rounded-md px-3 py-2 text-sm font-medium text-gray-600 hover:bg-gray-100 hover:text-gray-900 data-[state=active]:bg-blue-50 data-[state=active]:text-blue-700 data-[state=active]:shadow-none"
              >
                <Icon className="w-4 h-4" />
                {tab.label}
              </TabsTrigger>
            );
          })}
        </TabsList>
      </aside>

      {/* Scrollable content */}
      <div className="flex-1 min-w-0 overflow-y-auto">
        <div className="max-w-4xl mx-auto px-8 py-6">
          <TabsContent value="general">
            <PreferenceSettings />
          </TabsContent>
          <TabsContent value="calendar">
            <CalendarSettings />
          </TabsContent>
          <TabsContent value="recording">
            <RecordingSettings />
          </TabsContent>
          <TabsContent value="Transcriptionmodels">
            <TranscriptSettings
              transcriptModelConfig={transcriptModelConfig}
              setTranscriptModelConfig={setTranscriptModelConfig}
            />
          </TabsContent>
          <TabsContent value="summaryModels">
            <SummaryModelSettings />
          </TabsContent>
          <TabsContent value="beta">
            <BetaSettings />
          </TabsContent>
          <TabsContent value="about">
            <div className="bg-white rounded-lg border border-gray-200 shadow-sm">
              <About />
            </div>
          </TabsContent>
        </div>
      </div>
    </Tabs>
  );
};
