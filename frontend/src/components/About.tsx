import React, { useState, useEffect } from "react";
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import Image from 'next/image';
import { ExternalLink } from 'lucide-react';
import { APP_NAME, APP_TAG } from '@/lib/appName';

// Upstream project this app is built on (MIT License: keep this credit)
const UPSTREAM_NAME = 'Meetily';
const UPSTREAM_VERSION = '0.4.1';
const UPSTREAM_AUTHOR = 'Zackriya Solutions';
const UPSTREAM_URL = 'https://github.com/Zackriya-Solutions/meeting-minutes';
const UPSTREAM_LICENSE_URL = `${UPSTREAM_URL}/blob/main/LICENSE.md`;

const FEATURES = [
    { title: 'Stays on your computer', description: 'Recordings, transcripts and notes are stored locally. Nothing is uploaded unless you choose a cloud AI model.' },
    { title: 'Local transcription', description: 'Speech-to-text runs on your own machine with Whisper or Parakeet.' },
    { title: 'Your choice of AI', description: 'Generate notes with a local model or any OpenAI-compatible provider.' },
    { title: 'Calendar aware', description: 'Reminds you before meetings, notices calls, and names recordings after the event.' },
];

export function About() {
    const [currentVersion, setCurrentVersion] = useState<string>('');

    useEffect(() => {
        getVersion().then(setCurrentVersion).catch(console.error);
    }, []);

    const open = (url: string) => {
        invoke('open_external_url', { url }).catch(error => console.error('Failed to open link:', error));
    };

    return (
        <div className="p-6 space-y-6">
            <div className="text-center">
                <Image src="icon_128x128.png" alt={APP_NAME} width={64} height={64} className="mx-auto mb-3" />
                <h1 className="text-xl font-bold text-gray-900">{APP_NAME} {APP_TAG}</h1>
                {currentVersion && <p className="text-sm text-gray-500">Version {currentVersion}</p>}
                <p className="mt-2 text-sm text-gray-600">Private meeting notes that stay on your computer.</p>
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
                {FEATURES.map(feature => (
                    <div key={feature.title} className="rounded-lg border border-gray-100 bg-gray-50 p-3">
                        <p className="text-sm font-semibold text-gray-800">{feature.title}</p>
                        <p className="mt-1 text-xs text-gray-600">{feature.description}</p>
                    </div>
                ))}
            </div>

            <div className="border-t border-gray-200 pt-4 text-center text-xs text-gray-500 space-y-1">
                <p>
                    Built on{' '}
                    <button onClick={() => open(UPSTREAM_URL)} className="inline-flex items-center gap-0.5 text-blue-600 hover:underline">
                        {UPSTREAM_NAME} v{UPSTREAM_VERSION} <ExternalLink className="h-3 w-3" />
                    </button>{' '}
                    by {UPSTREAM_AUTHOR}.
                </p>
                <p>
                    Copyright © {UPSTREAM_AUTHOR}. Used under the{' '}
                    <button onClick={() => open(UPSTREAM_LICENSE_URL)} className="text-blue-600 hover:underline">
                        MIT License
                    </button>
                    .
                </p>
            </div>
        </div>
    );
}
