import { useEffect, useState } from 'react';
import { Badge, Button, ConfirmDialog } from '../../design/components';
import { useT } from '../../i18n';
import type { Provider, RepairReport, UpdateStatus } from '../../protocol/types';
import { useSyncClient } from '../../sync/react';
import s from './Settings.module.css';

const agentName: Partial<Record<Provider, string>> = { claude: 'Claude Code', gpt: 'Codex', antigravity: 'Antigravity', opencode: 'OpenCode', cursor: 'Cursor', droid: 'Droid' };

/** Updates and Repair Lumen: the two things that keep Lumen itself working. */
export function Maintenance() {
  return (
    <div className={s.section}>
      <Updates />
      <Repair />
    </div>
  );
}

function Updates() {
  const t = useT();
  const sync = useSyncClient();
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [busy, setBusy] = useState<'checking' | 'installing' | null>(null);
  const [checked, setChecked] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [back, setBack] = useState(false);
  const load = () => sync.call<UpdateStatus>('update_status').then(setStatus, () => {});
  useEffect(() => void load(), [sync]);
  if (!status) return null;
  const run = (command: string) => {
    setError(null);
    setBusy('installing');
    sync.call(command).catch((e) => {
      setBusy(null);
      setError(String(e));
    });
  };
  const available = status.available;
  return (
    <div className={s.api}>
      <h3 className={s.subtitle}>{t.maintenance.updates}</h3>
      <p className={s.quiet}>{t.general.version(status.current)}</p>
      {available ? <p role="status">{t.maintenance.available(available.version)}</p> : checked && <p className={s.quiet} role="status">{t.maintenance.upToDate}</p>}
      {available && !available.installable && <p className={s.help}>{t.maintenance.manual}</p>}
      {busy === 'installing' && <p className={s.quiet} role="status">{t.maintenance.installing}</p>}
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      <div className={s.actions}>
        {available?.installable ? (
          <Button variant="primary" isDisabled={busy !== null} onPress={() => run('update_install')}>
            {t.maintenance.install}
          </Button>
        ) : available ? (
          <Button variant="primary" onPress={() => sync.call('update_open_page').catch((e) => setError(String(e)))}>
            {t.maintenance.releasePage}
          </Button>
        ) : null}
        <Button
          variant="secondary"
          isDisabled={busy !== null}
          onPress={() => {
            setError(null);
            setBusy('checking');
            sync
              .call('update_check')
              .then(
                () => load().then(() => setChecked(true)),
                (e) => setError(String(e)),
              )
              .finally(() => setBusy(null));
          }}
        >
          {busy === 'checking' ? t.maintenance.checking : t.maintenance.check}
        </Button>
        {status.previous && status.installable && (
          <Button variant="secondary" isDisabled={busy !== null} onPress={() => setBack(true)}>
            {t.maintenance.goBack(status.previous)}
          </Button>
        )}
      </div>
      <p className={s.help}>{t.maintenance.updateHelp}</p>
      {status.previous && (
        <ConfirmDialog
          isOpen={back}
          onOpenChange={(o) => !o && setBack(false)}
          title={t.maintenance.goBackTitle(status.previous)}
          body={t.maintenance.goBackBody}
          confirmLabel={t.maintenance.goBack(status.previous)}
          cancelLabel={t.cancel}
          onConfirm={() => run('update_back')}
        />
      )}
    </div>
  );
}

/** Waits until the service answers again after a restart. */
async function answers(call: () => Promise<unknown>, tries = 60) {
  for (let i = 0; i < tries; i++) {
    try {
      await call();
      return true;
    } catch {
      await new Promise((r) => setTimeout(r, 500));
    }
  }
  return false;
}

function Repair() {
  const t = useT();
  const sync = useSyncClient();
  const [report, setReport] = useState<RepairReport | null>(null);
  const [restarted, setRestarted] = useState(false);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<RepairReport['tasks'][number] | null>(null);

  async function repair() {
    setRunning(true);
    setError(null);
    setReport(null);
    try {
      // A service that does not answer is restarted by the shell first.
      const status = () => sync.call('__status');
      let restart = false;
      try {
        await status();
      } catch {
        restart = true;
        await sync.call('restart_service');
        if (!(await answers(status))) throw t.banner.failed;
      }
      setRestarted(restart);
      setReport(await sync.call<RepairReport>('repair'));
      await sync.resync();
    } catch (e) {
      setError(t.maintenance.failed(String(e)));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className={s.api}>
      <h3 className={s.subtitle}>{t.maintenance.repair}</h3>
      <p className={s.help}>{t.maintenance.repairHelp}</p>
      <div className={s.actions}>
        <Button variant="primary" isDisabled={running} onPress={() => void repair()}>
          {running ? t.maintenance.repairing : t.maintenance.repair}
        </Button>
      </div>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      {/* Announced when it arrives: the result is the point of pressing Repair. */}
      <div role="status">
        {report && (
          <ul className={s.report} aria-label={t.maintenance.repair}>
            <li>
              <Badge tone="success">{restarted ? t.maintenance.serviceRestarted : t.maintenance.serviceOk}</Badge>
            </li>
            <li>{!report.database.ok ? <Badge tone="danger">{t.maintenance.databaseBad(report.database.detail ?? '')}</Badge> : report.database.backup ? <Badge tone="success">{t.maintenance.databaseOk}</Badge> : <Badge tone="warning">{t.maintenance.backupFailed(report.database.detail ?? '')}</Badge>}</li>
            <li>
              <Badge tone={report.recovered ? 'warning' : 'success'}>{report.recovered ? t.maintenance.recovered(report.recovered) : t.maintenance.nothingToRecover}</Badge>
            </li>
            {report.restored > 0 && (
              <li>
                <Badge tone="warning">{t.maintenance.restored(report.restored)}</Badge>
              </li>
            )}
            {report.accounts.map((a) => {
              const name = `${a.name} (${agentName[a.provider] ?? a.provider})`;
              return (
                <li key={a.id}>
                  <Badge tone={a.installed && a.signedIn ? 'success' : 'warning'}>{a.installed ? t.maintenance.agent(name, a.version, a.signedIn) : t.maintenance.agentMissing(name)}</Badge>
                </li>
              );
            })}
            {report.tasks.length === 0 ? (
              <li>
                <Badge tone="success">{t.maintenance.noLeftovers}</Badge>
              </li>
            ) : (
              report.tasks.map((task) => (
                <li key={task.path} className={s.leftover}>
                  <span>
                    <code>{task.path}</code>
                    <small>{t.maintenance.leftover(task.changes, task.branch)}</small>
                  </span>
                  <Button variant="secondary" onPress={() => setRemoving(task)}>
                    {t.maintenance.remove}
                  </Button>
                </li>
              ))
            )}
          </ul>
        )}
      </div>
      <ConfirmDialog
        isOpen={removing !== null}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={t.maintenance.removeTitle}
        body={t.maintenance.removeBody(removing ? removing.changes : 0)}
        confirmLabel={t.maintenance.remove}
        cancelLabel={t.cancel}
        danger
        onConfirm={() => {
          const task = removing;
          if (!task) return;
          sync.call('repair_remove_task', { path: task.path }).then(
            () => setReport((r) => (r ? { ...r, tasks: r.tasks.filter((x) => x.path !== task.path) } : r)),
            (e) => setError(t.maintenance.failed(String(e))),
          );
        }}
      />
    </div>
  );
}
