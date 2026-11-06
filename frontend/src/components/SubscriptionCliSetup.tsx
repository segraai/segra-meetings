'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AlertTriangle, Check, Copy, ExternalLink, Loader2, RefreshCw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

/** The command-line tools that let people summarize with their own plan */
export type SubscriptionCli = 'claude' | 'codex' | 'gemini';

/** Summary provider id for each CLI */
export const SUBSCRIPTION_CLI_PROVIDERS = {
  'claude-cli': 'claude',
  'codex-cli': 'codex',
  'gemini-cli': 'gemini',
} as const satisfies Record<string, SubscriptionCli>;

export type SubscriptionCliProvider = keyof typeof SUBSCRIPTION_CLI_PROVIDERS;

export function subscriptionCliFor(provider: string): SubscriptionCli | null {
  return (SUBSCRIPTION_CLI_PROVIDERS as Record<string, SubscriptionCli>)[provider] ?? null;
}

interface CliStatus {
  installed: boolean;
  path: string | null;
  version: string | null;
  loggedIn: boolean;
  usesSubscription: boolean;
  authLabel: string | null;
  plan: string | null;
  email: string | null;
  installDocsUrl: string;
  error: string | null;
}

const isWindows = typeof navigator !== 'undefined' && /win/i.test(navigator.platform);

const CLI_INFO: Record<SubscriptionCli, {
  toolName: string;
  title: string;
  description: string;
  company: string;
  installCommand: string;
  installNote?: string;
  signInLabel: string;
  terminalSignIn: string;
  terminalSignInNote?: string;
  wrongAccountWarning: string;
  extraNote?: string;
}> = {
  claude: {
    toolName: 'Claude Code',
    title: 'Use your Claude subscription',
    description: 'Summaries run through Claude Code on this computer, signed in with your Claude Pro or Max plan.',
    company: 'Anthropic',
    installCommand: isWindows ? 'irm https://claude.ai/install.ps1 | iex' : 'curl -fsSL https://claude.ai/install.sh | bash',
    signInLabel: 'Sign in with Claude',
    terminalSignIn: 'claude auth login',
    wrongAccountWarning:
      'Claude Code is signed in with an Anthropic Console account, which is billed per use rather than through your subscription. Sign in with your Claude account instead.',
  },
  codex: {
    toolName: 'Codex',
    title: 'Use your ChatGPT plan',
    description: 'Summaries run through OpenAI\'s Codex CLI on this computer, signed in with your ChatGPT account.',
    company: 'OpenAI',
    installCommand: 'npm install -g @openai/codex',
    installNote: 'Needs Node.js 20 or newer.',
    signInLabel: 'Sign in with ChatGPT',
    terminalSignIn: 'codex login',
    wrongAccountWarning:
      'Codex is signed in with an OpenAI API key, which is billed per use rather than through your ChatGPT plan. Sign in with your ChatGPT account instead.',
    extraNote: 'Codex availability and limits depend on your ChatGPT plan.',
  },
  gemini: {
    toolName: 'Gemini CLI',
    title: 'Use your Google account',
    description: 'Summaries run through Google\'s Gemini CLI on this computer, signed in with your Google account. Google AI plans raise the usage limits.',
    company: 'Google',
    installCommand: 'npm install -g @google/gemini-cli',
    installNote: 'Needs Node.js 20 or newer.',
    signInLabel: 'Sign in with Google',
    terminalSignIn: 'gemini',
    terminalSignInNote: 'then choose "Sign in with Google"',
    wrongAccountWarning: 'Gemini CLI is set up with an API key. Sign in with your Google account instead.',
    extraNote: 'On the free tier, Google may use prompts to improve its products. Check Google\'s terms for your plan.',
  },
};

interface SubscriptionCliSetupProps {
  cli: SubscriptionCli;
  model: string;
  /** Reports whether summaries can run (installed and signed in with a plan) */
  onReadyChange?: (ready: boolean) => void;
}

const URL_PATTERN = /https?:\/\/\S+/;

function CopyableCommand({ command }: { command: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex items-center gap-2 rounded-md border border-gray-200 bg-gray-50 px-3 py-2">
      <code className="min-w-0 flex-1 truncate font-mono text-xs text-gray-800">{command}</code>
      <button
        type="button"
        onClick={async () => {
          await navigator.clipboard.writeText(command);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
        className="shrink-0 text-gray-500 hover:text-gray-900"
        title="Copy"
      >
        {copied ? <Check className="h-4 w-4 text-blue-600" /> : <Copy className="h-4 w-4" />}
      </button>
    </div>
  );
}

function Step({ number, title, done, children }: { number: number; title: string; done: boolean; children: React.ReactNode }) {
  return (
    <div className="flex gap-3">
      <div
        className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold ${done ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700'}`}
      >
        {done ? <Check className="h-3.5 w-3.5" /> : number}
      </div>
      <div className="min-w-0 flex-1 space-y-2 pb-1">
        <p className="text-sm font-medium text-gray-900">{title}</p>
        {children}
      </div>
    </div>
  );
}

/**
 * Setup guide for summarizing with the user's own plan through its CLI
 * (Claude Code, Codex, Gemini CLI): install, sign in, test.
 */
export function SubscriptionCliSetup({ cli, model, onReadyChange }: SubscriptionCliSetupProps) {
  const info = CLI_INFO[cli];
  const [status, setStatus] = useState<CliStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [login, setLogin] = useState<{ running: boolean; lines: string[]; error?: string }>({ running: false, lines: [] });
  const [code, setCode] = useState('');
  const [test, setTest] = useState<{ state: 'idle' | 'running' | 'ok' | 'error'; message?: string }>({ state: 'idle' });
  const onReadyChangeRef = useRef(onReadyChange);
  onReadyChangeRef.current = onReadyChange;

  const refresh = useCallback(async () => {
    setChecking(true);
    try {
      setStatus(await invoke<CliStatus>('subscription_cli_status', { cli }));
    } catch (error) {
      console.error(`Failed to check ${cli}:`, error);
    } finally {
      setChecking(false);
    }
  }, [cli]);

  // Check now, and again whenever the user comes back to the app (e.g. after
  // installing or signing in from a terminal)
  useEffect(() => {
    setStatus(null);
    setTest({ state: 'idle' });
    setLogin({ running: false, lines: [] });
    refresh();
    window.addEventListener('focus', refresh);
    return () => window.removeEventListener('focus', refresh);
  }, [refresh]);

  const signedIn = !!status?.loggedIn && status.usesSubscription;
  // Signed in with an API key: works, but billed per use, so it still needs a plan sign-in
  const wrongAccount = !!status?.loggedIn && !status.usesSubscription;
  // A working test counts as signed in, even when the status check can't confirm it
  const ready = !!status?.installed && (signedIn || (test.state === 'ok' && !wrongAccount));
  useEffect(() => {
    onReadyChangeRef.current?.(ready);
  }, [ready]);

  // Sign-in output and completion
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    let cancelled = false;
    (async () => {
      const offOutput = await listen<string>('subscription-cli-login-output', ({ payload }) => {
        setLogin(prev => ({ ...prev, lines: [...prev.lines, payload].slice(-8) }));
      });
      const offFinished = await listen<{ cli: SubscriptionCli; success: boolean }>('subscription-cli-login-finished', ({ payload }) => {
        if (payload.cli !== cli) return;
        setLogin(prev => ({
          running: false,
          lines: prev.lines,
          error: payload.success ? undefined : 'Sign-in did not complete. Try again, or sign in from a terminal.',
        }));
        setCode('');
        refresh();
      });
      if (cancelled) {
        offOutput();
        offFinished();
      } else {
        unlisteners.push(offOutput, offFinished);
      }
    })();
    return () => {
      cancelled = true;
      unlisteners.forEach(off => off());
    };
  }, [cli, refresh]);

  // Stop a sign-in left running when the settings close or the provider changes
  useEffect(() => () => { void invoke('subscription_cli_login_cancel'); }, [cli]);

  const startLogin = async () => {
    setLogin({ running: true, lines: [] });
    try {
      await invoke('subscription_cli_login_start', { cli });
    } catch (error) {
      setLogin({ running: false, lines: [], error: String(error) });
    }
  };

  const cancelLogin = async () => {
    await invoke('subscription_cli_login_cancel');
    setLogin({ running: false, lines: [] });
  };

  const submitCode = async () => {
    if (!code.trim()) return;
    try {
      await invoke('subscription_cli_login_submit_code', { code });
    } catch (error) {
      setLogin(prev => ({ ...prev, error: String(error) }));
    }
  };

  const runTest = useCallback(async () => {
    setTest({ state: 'running' });
    try {
      await invoke<string>('subscription_cli_test', { cli, model });
      setTest({ state: 'ok' });
    } catch (error) {
      setTest({ state: 'error', message: String(error) });
    }
  }, [cli, model]);

  // When the status check doesn't confirm a sign-in, try the CLI once before asking
  // the user to sign in: it may already work
  const autoTested = useRef(false);
  useEffect(() => {
    autoTested.current = false;
  }, [cli]);
  useEffect(() => {
    if (!status?.installed || signedIn || wrongAccount || autoTested.current) return;
    autoTested.current = true;
    void runTest();
  }, [status, signedIn, wrongAccount, runTest]);

  const openUrl = (url: string) => {
    invoke('open_external_url', { url }).catch(error => console.error('Failed to open link:', error));
  };

  const signInUrl = login.lines.map(line => line.match(URL_PATTERN)?.[0]).filter(Boolean).pop();
  const plan = status?.plan ? status.plan.charAt(0).toUpperCase() + status.plan.slice(1) : null;

  if (!status) {
    return (
      <div className="flex items-center gap-2 py-4 text-sm text-gray-500">
        <Loader2 className="h-4 w-4 animate-spin" /> Checking for {info.toolName}...
      </div>
    );
  }

  return (
    <div className="space-y-4 border-t pt-4">
      <div className="space-y-1">
        <p className="text-sm font-medium text-gray-900">{info.title}</p>
        <p className="text-xs text-gray-600">
          {info.description} No API key is needed and usage counts toward your plan&apos;s limits. Transcripts and
          notes are sent to {info.company} to be summarized.
        </p>
        {info.extraNote && <p className="text-xs text-gray-500">{info.extraNote}</p>}
      </div>

      <Step number={1} title={`Install ${info.toolName}`} done={status.installed}>
        {status.installed ? (
          <p className="text-xs text-gray-600">
            Installed{status.version ? ` · version ${status.version}` : ''}.
          </p>
        ) : (
          <>
            <p className="text-xs text-gray-600">
              Open a terminal, run this command, then come back here.{info.installNote ? ` ${info.installNote}` : ''}
            </p>
            <CopyableCommand command={info.installCommand} />
            <button
              type="button"
              onClick={() => openUrl(status.installDocsUrl)}
              className="inline-flex items-center gap-1 text-xs text-blue-700 hover:underline"
            >
              Installation guide <ExternalLink className="h-3 w-3" />
            </button>
          </>
        )}
      </Step>

      <Step number={2} title="Sign in" done={ready}>
        {!status.installed ? (
          <p className="text-xs text-gray-500">Available once {info.toolName} is installed.</p>
        ) : signedIn ? (
          <p className="text-xs text-gray-600">
            Signed in{status.email ? ` as ${status.email}` : ''}{plan ? ` · ${plan} plan` : ''}.
          </p>
        ) : ready ? (
          <p className="text-xs text-gray-600">Already signed in: {info.toolName} answered the test.</p>
        ) : test.state === 'running' && !wrongAccount ? (
          <p className="flex items-center gap-2 text-xs text-gray-600">
            <Loader2 className="h-3.5 w-3.5 animate-spin" /> Checking whether {info.toolName} is already signed in...
          </p>
        ) : (
          <>
            {wrongAccount && (
              <p className="flex items-start gap-1.5 rounded-md bg-amber-50 px-2 py-1.5 text-xs text-amber-800">
                <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                {info.wrongAccountWarning}
              </p>
            )}
            {!login.running ? (
              <Button type="button" size="sm" onClick={startLogin}>
                {info.signInLabel}
              </Button>
            ) : (
              <div className="space-y-2 rounded-md border border-gray-200 p-3">
                <p className="flex items-center gap-2 text-xs text-gray-700">
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  Finish signing in in your browser.
                </p>
                {signInUrl && (
                  <button
                    type="button"
                    onClick={() => openUrl(signInUrl)}
                    className="inline-flex items-center gap-1 text-xs text-blue-700 hover:underline"
                  >
                    Open the sign-in page {login.lines.length > 0 ? 'again' : ''} <ExternalLink className="h-3 w-3" />
                  </button>
                )}
                <div className="flex gap-2">
                  <Input
                    value={code}
                    onChange={e => setCode(e.target.value)}
                    placeholder="If the page shows a code, paste it here"
                    className="h-8 text-xs"
                  />
                  <Button type="button" size="sm" variant="outline" onClick={submitCode} disabled={!code.trim()}>
                    Submit
                  </Button>
                </div>
                <button type="button" onClick={cancelLogin} className="text-xs text-gray-500 hover:text-gray-800">
                  Cancel
                </button>
              </div>
            )}
            {login.error && <p className="text-xs text-red-600">{login.error}</p>}
            <p className="text-xs text-gray-500">
              Or sign in from a terminal{info.terminalSignInNote ? ` (${info.terminalSignInNote})` : ''}, then come back
              here:
            </p>
            <CopyableCommand command={info.terminalSignIn} />
          </>
        )}
      </Step>

      <Step number={3} title="Test it" done={test.state === 'ok'}>
        {!status.installed ? (
          <p className="text-xs text-gray-500">Available once {info.toolName} is installed.</p>
        ) : (
          <div className="flex flex-wrap items-center gap-3">
            <Button type="button" size="sm" variant="outline" onClick={runTest} disabled={test.state === 'running'}>
              {test.state === 'running' && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
              Test connection
            </Button>
            {test.state === 'ok' && <span className="text-xs text-blue-700">Working. Summaries will use {model || 'the default model'}.</span>}
            {test.state === 'error' && <span className="text-xs text-red-600">{test.message}</span>}
          </div>
        )}
      </Step>

      {status.error && <p className="text-xs text-red-600">{status.error}</p>}

      <button
        type="button"
        onClick={refresh}
        disabled={checking}
        className="inline-flex items-center gap-1 text-xs text-gray-500 hover:text-gray-800"
      >
        <RefreshCw className={`h-3 w-3 ${checking ? 'animate-spin' : ''}`} /> Check again
      </button>
    </div>
  );
}
