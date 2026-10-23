'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Mic, Volume2 } from 'lucide-react';

interface RecordingAudioLevels {
  mic_rms: number;
  mic_peak: number;
  system_rms: number;
  system_peak: number;
}

const POLL_MS = 70;
const BAR_COUNT = 32;
const SILENT = new Array<number>(BAR_COUNT).fill(0);

/** Maps an RMS level onto 0..1 over a -60 dB..0 dB range, so speech fills the meter. */
function toDisplayLevel(rms: number): number {
  if (rms <= 0) return 0;
  const db = 20 * Math.log10(rms);
  return Math.min(1, Math.max(0, (db + 60) / 60));
}

function Waveform({ levels, barClassName }: { levels: number[]; barClassName: string }) {
  return (
    <div className="flex h-6 flex-1 items-center gap-[2px]" aria-hidden="true">
      {levels.map((level, i) => (
        <div
          key={i}
          className={`w-[3px] shrink-0 rounded-full transition-[height] duration-75 ${level > 0.02 ? barClassName : 'bg-gray-300'}`}
          style={{ height: `${Math.max(8, level * 100)}%` }}
        />
      ))}
    </div>
  );
}

interface RecordingLevelMetersProps {
  isPaused?: boolean;
}

/**
 * Live waveforms of the microphone and the meeting (system) audio being recorded,
 * so it's visible at a glance that both are actually captured.
 */
export function RecordingLevelMeters({ isPaused = false }: RecordingLevelMetersProps) {
  const [mic, setMic] = useState<number[]>(SILENT);
  const [system, setSystem] = useState<number[]>(SILENT);

  useEffect(() => {
    if (isPaused) {
      setMic(SILENT);
      setSystem(SILENT);
      return;
    }

    let cancelled = false;
    const timer = setInterval(async () => {
      try {
        const levels = await invoke<RecordingAudioLevels>('get_recording_audio_levels');
        if (cancelled) return;
        setMic(prev => [...prev.slice(1), toDisplayLevel(levels.mic_rms)]);
        setSystem(prev => [...prev.slice(1), toDisplayLevel(levels.system_rms)]);
      } catch {
        // Not recording any more: the parent unmounts us shortly
      }
    }, POLL_MS);

    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [isPaused]);

  return (
    <div className="flex min-w-0 flex-1 flex-col gap-1">
      <div className="flex items-center gap-2" title="Your microphone">
        <Mic className="h-3.5 w-3.5 shrink-0 text-gray-500" />
        <span className="w-24 shrink-0 text-xs text-gray-600">You</span>
        <Waveform levels={mic} barClassName="bg-blue-500" />
      </div>
      <div className="flex items-center gap-2" title="Meeting audio (what you hear)">
        <Volume2 className="h-3.5 w-3.5 shrink-0 text-gray-500" />
        <span className="w-24 shrink-0 text-xs text-gray-600">Meeting audio</span>
        <Waveform levels={system} barClassName="bg-[#0b87a6]" />
      </div>
    </div>
  );
}
