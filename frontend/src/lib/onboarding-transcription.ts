import { invoke } from '@tauri-apps/api/core';
import { getProviderCommands, type ModelWithStatus } from '@/lib/transcription-model-readiness';

/** Transcription engines onboarding can set up, keyed by transcript config provider */
export type TranscriptionEngine = 'parakeet' | 'localWhisper';

export interface TranscriptionEngineOption {
  engine: TranscriptionEngine;
  model: string;
  label: string;
  sizeMb: number;
  description: string;
}

export const TRANSCRIPTION_ENGINE_OPTIONS: Record<TranscriptionEngine, TranscriptionEngineOption> = {
  parakeet: {
    engine: 'parakeet',
    model: 'parakeet-tdt-0.6b-v3-int8',
    label: 'Parakeet',
    sizeMb: 670,
    description: 'Fast and accurate. The best choice for most meetings.',
  },
  localWhisper: {
    engine: 'localWhisper',
    model: 'large-v3-turbo-q5_0',
    label: 'Whisper',
    sizeMb: 547,
    description: 'Supports around 100 languages. A little slower than Parakeet.',
  },
};

export const DEFAULT_TRANSCRIPTION_ENGINE: TranscriptionEngine = 'parakeet';

export function toTranscriptionEngine(value: string | null | undefined): TranscriptionEngine {
  return value === 'localWhisper' ? 'localWhisper' : DEFAULT_TRANSCRIPTION_ENGINE;
}

/** Whether the engine's onboarding model is already downloaded */
export async function isTranscriptionModelOnDisk(engine: TranscriptionEngine): Promise<boolean> {
  const commands = getProviderCommands(engine);
  if (!commands) return false;
  await invoke(commands.initialize);
  const models = await invoke<Array<ModelWithStatus & { name: string }>>(commands.getAvailableModels);
  const model = TRANSCRIPTION_ENGINE_OPTIONS[engine].model;
  return models.some((m) => m.name === model && m.status === 'Available');
}

/** Download the engine's onboarding model. Resolves when the download ends;
 * progress arrives as events. */
export async function downloadTranscriptionModel(engine: TranscriptionEngine): Promise<void> {
  const { model } = TRANSCRIPTION_ENGINE_OPTIONS[engine];
  if (engine === 'localWhisper') {
    await invoke('whisper_init');
    await invoke('whisper_download_model', { modelName: model });
  } else {
    await invoke('parakeet_download_model', { modelName: model });
  }
}

/** Retry a failed download of the engine's onboarding model */
export async function retryTranscriptionModelDownload(engine: TranscriptionEngine): Promise<void> {
  if (engine === 'localWhisper') {
    // Whisper has no separate retry command, so start the download again
    await downloadTranscriptionModel(engine);
  } else {
    await invoke('parakeet_retry_download', { modelName: TRANSCRIPTION_ENGINE_OPTIONS.parakeet.model });
  }
}
