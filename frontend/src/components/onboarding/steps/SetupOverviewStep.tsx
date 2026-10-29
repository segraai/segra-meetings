import React, { useEffect, useState } from 'react';
import { Check, Info } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { TRANSCRIPTION_ENGINE_OPTIONS, type TranscriptionEngine } from '@/lib/onboarding-transcription';

const ENGINE_CHOICES: TranscriptionEngine[] = ['parakeet', 'localWhisper'];

export function SetupOverviewStep() {
  const { goNext, transcriptionEngine, setTranscriptionEngine } = useOnboarding();
  const [isMac, setIsMac] = useState(false);

  useEffect(() => {
    const checkPlatform = async () => {
      try {
        const { platform } = await import('@tauri-apps/plugin-os');
        setIsMac(platform() === 'macos');
      } catch (e) {
        setIsMac(navigator.userAgent.includes('Mac'));
      }
    };
    checkPlatform();
  }, []);

  const steps = [
    {
      number: 1,
      type: 'transcription',
      title: 'Download Transcription Engine',
    },
    {
      number: 2,
      type: 'summarization',
      title: 'Download Summarization Engine',
    },
  ];

  const handleContinue = () => {
    goNext();
  };

  return (
    <OnboardingContainer
      title="Setup Overview"
      description="Segra Meetings requires that you download the Transcription & Summarization AI models for the software to work."
      step={2}
      totalSteps={isMac ? 4 : 3}
    >
      <div className="flex flex-col items-center space-y-10">
        {/* Steps Card */}
        <div className="w-full max-w-md bg-white rounded-lg border border-gray-200 p-4">
          <div className="space-y-4">
            {steps.map((step, idx) => {
              return (
                <div
                  key={step.number}
                  className={`flex items-start gap-4 p-1`}
                >
                  <div className="flex-1 ml-1">
                    <h3 className="font-medium text-gray-900 flex items-center gap-2">
                        Step {step.number} :  {step.title}

                        {step.type === "summarization" && (
                            <TooltipProvider>
                            <Tooltip>
                                <TooltipTrigger asChild>
                                <button className="text-gray-400 hover:text-gray-600">
                                    <Info className="w-4 h-4" />
                                </button>
                                </TooltipTrigger>
                                <TooltipContent className="max-w-xs text-sm">
                                You can also select external AI providers like OpenAI, Claude, or
                                Ollama for summary generation in settings.
                                </TooltipContent>
                            </Tooltip>
                            </TooltipProvider>
                        )}
                        </h3>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Transcription engine choice */}
        <div className="w-full max-w-md space-y-2">
          <h3 className="text-sm font-medium text-gray-900">Transcription model</h3>
          <div className="space-y-2" role="radiogroup" aria-label="Transcription model">
            {ENGINE_CHOICES.map((engine) => {
              const option = TRANSCRIPTION_ENGINE_OPTIONS[engine];
              const selected = transcriptionEngine === engine;
              return (
                <button
                  key={engine}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  onClick={() => setTranscriptionEngine(engine)}
                  className={`w-full text-left rounded-lg border p-3 flex items-start gap-3 transition-colors ${
                    selected ? 'border-gray-900 bg-gray-50' : 'border-gray-200 bg-white hover:border-gray-400'
                  }`}
                >
                  <div
                    className={`mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center flex-shrink-0 ${
                      selected ? 'border-gray-900 bg-gray-900' : 'border-gray-300'
                    }`}
                  >
                    {selected && <Check className="w-3 h-3 text-white" />}
                  </div>
                  <div>
                    <p className="text-sm font-medium text-gray-900">
                      {option.label}
                      {engine === 'parakeet' && <span className="ml-2 text-xs font-normal text-gray-500">Default</span>}
                      <span className="ml-2 text-xs font-normal text-gray-500">~{option.sizeMb} MB</span>
                    </p>
                    <p className="text-xs text-gray-600 mt-0.5">{option.description}</p>
                  </div>
                </button>
              );
            })}
          </div>
          <p className="text-xs text-gray-500">
            If you will record meetings in other languages, use Whisper. You can change this later in Settings.
          </p>
        </div>

        {/* CTA Section */}
        <div className="w-full max-w-xs space-y-4">
          <Button
            onClick={handleContinue}
            className="w-full h-11 bg-gray-900 hover:bg-gray-800 text-white"
          >
            Let's Go
          </Button>
          <div className="text-center">
            <a
              href="https://github.com/Zackriya-Solutions/meeting-minutes"
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-gray-600 hover:underline"
            >
              Report issues on GitHub
            </a>
          </div>
        </div>
      </div>
    </OnboardingContainer>
  );
}
