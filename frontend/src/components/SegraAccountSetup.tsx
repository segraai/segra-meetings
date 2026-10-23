'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CheckCircle2, ExternalLink, Loader2, LogOut, ShieldCheck } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface SegraAccount {
  signedIn: boolean;
  userId: string | null;
  server: string;
}

/** Every Segra account has its own wallet and a balance in credits. */
interface SegraBalance {
  credits: number | null;
  status: 'ok' | 'setting_up' | 'empty' | null;
}

function balanceText(b: SegraBalance) {
  if (b.status === 'setting_up') return 'Setting up your account…';
  if (b.status === 'empty') return 'Balance used up. Top up your Segra account to keep going.';
  if (b.credits !== null) {
    return `${b.credits.toLocaleString(undefined, { maximumFractionDigits: b.credits < 10 ? 2 : 0 })} credits left`;
  }
  return null;
}

interface SignInStart {
  userCode: string;
  verificationUri: string;
  verificationUriComplete: string;
  expiresIn: number;
}

/** Segra account panel for the "segra" summary provider: device sign-in, the signed-in account and
 *  its credits. The API key stays in the Rust side; this only sees the account ID. */
export function SegraAccountSetup({ onReadyChange }: { onReadyChange?: (ready: boolean) => void }) {
  const [account, setAccount] = useState<SegraAccount | null>(null);
  const [balance, setBalance] = useState<SegraBalance | null>(null);
  const [pending, setPending] = useState<SignInStart | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const loadBalance = useCallback(() => {
    invoke<SegraBalance | null>('segra_get_balance')
      .then(setBalance)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    invoke<SegraAccount>('segra_get_account')
      .then((a) => {
        setAccount(a);
        if (a.signedIn) loadBalance();
      })
      .catch((e) => setError(String(e)));
  }, [loadBalance]);

  useEffect(() => {
    onReadyChange?.(!!account?.signedIn);
  }, [account?.signedIn, onReadyChange]);

  const signIn = async () => {
    setError(null);
    setBusy(true);
    try {
      const start = await invoke<SignInStart>('segra_start_sign_in');
      setPending(start);
      const signedIn = await invoke<SegraAccount>('segra_finish_sign_in');
      setAccount(signedIn);
      loadBalance();
    } catch (e) {
      const message = String(e);
      if (!message.includes('cancelled')) setError(message);
    } finally {
      setPending(null);
      setBusy(false);
    }
  };

  const cancel = () => {
    void invoke('segra_cancel_sign_in');
  };

  const signOut = async () => {
    setError(null);
    try {
      await invoke('segra_sign_out');
      setAccount((a) => (a ? { ...a, signedIn: false, userId: null } : a));
      setBalance(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const openPage = (url: string) => {
    invoke('open_external_url', { url }).catch((e) => console.error('Failed to open link:', e));
  };

  return (
    <div className="space-y-3 border-t pt-4">
      <div className="flex items-start gap-2 text-sm text-muted-foreground">
        <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0" />
        <p>
          Segra models run inside secure hardware (TEEs). Your transcript goes straight from this computer to the
          model, never through Segra, and usage is paid from your Segra account.
        </p>
      </div>

      {account === null && !error && (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" /> Checking your Segra account…
        </p>
      )}

      {account?.signedIn && (
        <div className="flex items-center justify-between gap-3 rounded-md border p-3">
          <div className="flex min-w-0 items-center gap-2">
            <CheckCircle2 className="h-4 w-4 shrink-0 text-green-600" />
            <div className="min-w-0 text-sm">
              <p className="truncate">
                Signed in as <span className="font-medium">{account.userId ?? 'your Segra account'}</span>
              </p>
              {balance && balanceText(balance) && (
                <p className={`text-xs ${balance.status === 'empty' ? 'text-red-600' : 'text-muted-foreground'}`}>
                  {balanceText(balance)}
                </p>
              )}
            </div>
          </div>
          <Button variant="outline" size="sm" onClick={() => void signOut()}>
            <LogOut className="mr-1 h-4 w-4" /> Sign out
          </Button>
        </div>
      )}

      {account && !account.signedIn && !pending && (
        <Button onClick={() => void signIn()} disabled={busy}>
          {busy && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
          Sign in with Segra
        </Button>
      )}

      {pending && (
        <div className="space-y-2 rounded-md border p-3 text-sm">
          <p>Approve the sign-in in your browser. Check that it shows this code:</p>
          <p className="text-center font-mono text-2xl tracking-widest">{pending.userCode}</p>
          <p className="flex items-center gap-2 text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Waiting for approval…
          </p>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => openPage(pending.verificationUriComplete)}>
              <ExternalLink className="mr-1 h-4 w-4" /> Open the page again
            </Button>
            <Button variant="ghost" size="sm" onClick={cancel}>
              Cancel
            </Button>
          </div>
        </div>
      )}

      {error && <p className="text-sm text-red-600">{error}</p>}
    </div>
  );
}
