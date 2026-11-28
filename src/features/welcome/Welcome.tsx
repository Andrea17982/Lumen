import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Mark } from '../../brand/Logo';
import { Button } from '../../design/components';
import { Icon } from '../../design/icons';
import { useT } from '../../i18n';
import { isCancelled, type Account, type OnboardingAgent, type Provider, type Workspace } from '../../protocol/types';
import { errorText } from '../../transport/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import { folderName } from '../sessions/sessions';
import { providerName, SignInFlow } from '../settings/Accounts';
import { AppearanceFields, type Preferences } from '../settings/General';
import s from './Welcome.module.css';

const steps = ['title', 'appearance', 'agents', 'project', 'summary'] as const;

export function WelcomeFrame({ step, children, footer }: { step: number; children?: ReactNode; footer: ReactNode }) {
  const t = useT().welcome;
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => heading.current?.focus(), [step]);
  const descriptions = [t.intro, t.appearanceHelp, t.agentsHelp, t.projectHelp];
  return (
    <main className={s.screen} aria-labelledby="welcome-heading">
      <div className={s.frame}>
        <header className={s.header}>
          <div className={s.brand} aria-hidden="true"><Mark size={step === 0 ? 72 : 32} label="" />lumen</div>
          <div className={s.progress} role="progressbar" aria-label={t.progress(step + 1, steps.length)} aria-valuemin={1} aria-valuemax={steps.length} aria-valuenow={step + 1}>
            {steps.map((name, index) => <span key={name} data-reached={index <= step} />)}
          </div>
        </header>
        <section className={s.step} key={step}>
          <h1 id="welcome-heading" tabIndex={-1} ref={heading}>{t[steps[step]]}</h1>
          {descriptions[step] && <p className={s.description}>{descriptions[step]}</p>}
          {children}
        </section>
        <footer className={s.actions}>{footer}</footer>
      </div>
    </main>
  );
}

export function AppearancePreview({ preferences }: Parameters<typeof AppearanceFields>[0]) {
  const t = useT();
  return (
    <div className={s.appearance}>
      <div className={s.fields}><AppearanceFields preferences={preferences} /></div>
      <div className={s.preview} aria-hidden="true">
        <div className={s.previewChrome}><Mark label="" /><span>lumen</span></div>
        <div className={s.previewComposer}>
          <span>{t.welcome.sample}</span>
          <span className={s.previewActions}><span>{t.modeAuto}</span><span className={s.previewSend}><Icon name="up" /></span></span>
        </div>
      </div>
    </div>
  );
}

export function Welcome({ preferences, project, onProject, onClose }: { preferences: Preferences; project: Workspace | null; onProject: (id: string) => void; onClose: (tour: boolean) => void }) {
  const t = useT();
  const w = t.welcome;
  const sync = useSyncClient();
  const connection = useSyncState((state) => state.connection.state);
  const connected = connection === 'connected';
  const [step, setStep] = useState(0);
  const [agents, setAgents] = useState<OnboardingAgent[] | null>(null);
  const [scan, setScan] = useState(0);
  const [scanning, setScanning] = useState(false);
  const [scanError, setScanError] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveFailed, setSaveFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [signing, setSigning] = useState<Account | null>(null);
  const [terminal, setTerminal] = useState(false);
  const [picked, setPicked] = useState<Workspace | null>(null);
  const chosen = picked ?? project;

  useEffect(() => {
    if (step !== 2 || !connected) return;
    let active = true;
    setScanning(true);
    setScanError(false);
    sync.call<OnboardingAgent[]>('onboarding_agents').then(
      (results) => { if (active) setAgents(results); },
      () => { if (active) { setScanError(true); setAgents(null); } },
    ).finally(() => { if (active) setScanning(false); });
    return () => { active = false; setScanning(false); };
  }, [step, scan, connected, sync]);

  function move(next: number) {
    setError(null);
    setSaveFailed(false);
    setStep(next);
  }

  async function connect(provider: Provider, inApp: boolean, existing?: Account) {
    setBusy(true);
    setError(null);
    setTerminal(false);
    try {
      const account = existing ?? await sync.call<Account>('add_account', { name: providerName[provider], provider });
      await sync.loadAccounts();
      if (inApp) setSigning(account);
      else if (provider !== 'ollama') {
        await sync.call('login_account', { id: account.id });
        setTerminal(true);
      }
    } catch (e) {
      if (!isCancelled(e)) setError(errorText(e));
    } finally {
      setScan((value) => value + 1);
      setBusy(false);
    }
  }

  async function chooseFolder() {
    setError(null);
    setBusy(true);
    try {
      const folder = await sync.pickFolder();
      if (!folder) return;
      const added = await sync.mutate<Workspace>('add_workspace', { folder, name: folderName(folder) });
      setPicked(added);
      onProject(added.id);
    } catch (e) {
      if (!isCancelled(e)) setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  async function close(skipped: boolean, tour = false) {
    setBusy(true);
    setError(null);
    setSaveFailed(false);
    try {
      await sync.updateOnboarding(skipped ? 'skipSetup' : 'finishSetup');
      onClose(tour);
    } catch {
      setError(w.saveError);
      setSaveFailed(true);
    } finally {
      setBusy(false);
    }
  }

  const ready = agents?.filter((agent) => agent.installed && agent.accounts.some(({ signedIn }) => signedIn === true)) ?? [];
  const verified = connected && agents !== null && !scanning && !scanError;
  function accountStatus(signedIn: boolean | null) {
    return signedIn === true ? w.ready : signedIn === false ? w.signedOut : w.unknown;
  }

  return (
    <WelcomeFrame step={step} footer={
      <>
        {step > 0 && <Button variant="plain" isDisabled={busy} onPress={() => move(step - 1)}>{w.back}</Button>}
        <Button variant="plain" isDisabled={busy} onPress={() => void close(true)}>{w.skip}</Button>
        {saveFailed && <Button variant="secondary" onPress={() => onClose(false)}>{w.leave}</Button>}
        <span className={s.space} />
        {step === 4 ? (
          <>
            <Button variant="secondary" isDisabled={busy} onPress={() => void close(false)}>{w.finish}</Button>
            <Button variant="primary" isDisabled={busy} onPress={() => void close(false, true)}>{w.startTour}</Button>
          </>
        ) : <Button variant="primary" isDisabled={busy} onPress={() => move(step + 1)}>{step === 0 ? w.begin : w.next}</Button>}
      </>
    }>
      {step === 1 && <AppearancePreview preferences={preferences} />}
      {step === 2 && (
        <>
          <div className={s.scan}>
            <span role="status">{!connected ? t.connection[connection] : scanning || (!agents && !scanError) ? w.checking : terminal ? w.terminal : agents ? w.found(agents.filter((agent) => agent.installed).length) : ''}</span>
            <Button variant="secondary" icon="repeat" isDisabled={scanning || busy || !connected} onPress={() => setScan((value) => value + 1)}>{w.check}</Button>
          </div>
          {scanError && <p role="alert" className={s.error}>{w.checkFailed}</p>}
          {agents && <ul className={s.list}>{[...agents].sort((a, b) => Number(b.installed) - Number(a.installed)).map((agent) => (
            <li key={agent.provider}>
              <div className={s.row}>
                <Icon name="terminal" size={19} />
                <div><strong>{providerName[agent.provider]}</strong><small>{agent.installed ? [w.installed, agent.version, agent.accounts.length === 0 ? w.unconfigured : null].filter(Boolean).join(' · ') : w.missing}</small></div>
                {agent.installed && agent.accounts.length === 0 && (
                  <Button isDisabled={busy || scanning || !connected} variant="secondary" onPress={() => void connect(agent.provider, agent.inAppSignIn)}>
                    {agent.provider === 'ollama' ? t.accounts.add : t.accounts.signIn}
                  </Button>
                )}
              </div>
              {agent.installed && agent.accounts.map(({ account, signedIn }) => (
                <div className={s.account} key={account.id}>
                  <span>{account.name} · <span data-ready={signedIn === true}>{accountStatus(signedIn)}</span></span>
                  {signedIn !== true && agent.provider !== 'ollama' && (
                    <Button isDisabled={busy || scanning || !connected} variant="plain" onPress={() => void connect(agent.provider, agent.inAppSignIn, account)}>{t.accounts.signIn}</Button>
                  )}
                </div>
              ))}
            </li>
          ))}</ul>}
          {signing && <SignInFlow account={signing} onFinished={() => setScan((value) => value + 1)} onClose={() => setSigning(null)} />}
        </>
      )}
      {step === 3 && (
        <div className={s.details}>
          <div className={s.row}><Icon name="folder" size={22} /><div><strong>{chosen?.name ?? w.projectMissing}</strong>{chosen && <small>{chosen.folder}</small>}</div><Button variant="secondary" isDisabled={busy || !connected} onPress={() => void chooseFolder()}>{w.chooseFolder}</Button></div>
          <div className={s.row}><Icon name="info" size={22} /><div><strong>{w.routing}</strong><small>{w.routingHelp}</small></div></div>
        </div>
      )}
      {step === 4 && (
        <ul className={s.details}>
          <li className={s.row}><Icon name={verified && ready.length ? 'check' : 'info'} size={22} /><div><strong>{w.agents}</strong><small>{!verified ? w.agentsUnchecked : ready.length ? w.agentsReady(ready.map((agent) => providerName[agent.provider]).join(', ')) : w.agentsMissing}</small></div></li>
          <li className={s.row}><Icon name={chosen ? 'check' : 'info'} size={22} /><div><strong>{chosen ? w.projectReady(chosen.name) : w.projectMissing}</strong>{!chosen && <small>{w.projectLater}</small>}</div></li>
          <li className={s.row}><Icon name="check" size={22} /><div><strong>{w.routing}</strong><small>{w.routingHelp}</small></div></li>
        </ul>
      )}
      {error && <p role="alert" className={s.error}>{error}</p>}
    </WelcomeFrame>
  );
}
