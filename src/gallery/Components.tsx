import { useState } from 'react';
import { useLocale, useT, type Locale } from '../i18n';
import { formatNumber, formatPercent, formatRelative, formatTime } from '../i18n/format';
import { styles, type Appearance } from '../design/appearance';
import type { ThemePreference } from '../design/theme';
import { Icon, iconNames } from '../design/icons';
import { Badge, Button, ConfirmDialog, IconButton, Menu, SearchField, Segmented, Select, Switch, TextField } from '../design/components';
import s from './Components.module.css';

/** Every primitive in every state, for review and for the accessibility and screenshot tests. */
export function Components({
  appearance,
  setAppearance,
  setLocale,
}: {
  appearance: Appearance;
  setAppearance: (a: Partial<Appearance>) => void;
  setLocale: (l: Locale) => void;
}) {
  const t = useT();
  const locale = useLocale();
  const [verify, setVerify] = useState(true);
  const [query, setQuery] = useState('');
  const [name, setName] = useState('');
  const [last, setLast] = useState('—');
  const now = Date.now();
  return (
    <main className={s.page} aria-label="Components">
      <section>
        <h2>{t.appearance}</h2>
        <div className={s.row}>
          <div className={s.labelled}>
            <span className={s.caption}>{t.styleLabel}</span>
            <Segmented label={t.styleLabel} value={appearance.style} options={styles.map((id) => ({ id, label: t.styles[id] }))} onChange={(style) => setAppearance({ style })} />
          </div>
          <Select<ThemePreference>
            label={t.themeLabel}
            value={appearance.theme}
            options={(['system', 'light', 'dark'] as const).map((id) => ({ id, label: t.theme[id] }))}
            onChange={(theme) => setAppearance({ theme })}
          />
          <Select<Locale> label={t.languageLabel} value={locale} options={(['en', 'it'] as const).map((id) => ({ id, label: t.language[id] }))} onChange={setLocale} />
        </div>
      </section>

      <section>
        <h2>Buttons</h2>
        <div className={s.row}>
          <Button variant="primary" icon="plus">
            {t.newTask}
          </Button>
          <Button variant="secondary">{t.reject}</Button>
          <Button variant="plain">{t.cancel}</Button>
          <Button variant="danger">{t.delete}</Button>
          <Button variant="primary" isDisabled>
            {t.approve}
          </Button>
          <IconButton icon="sidebar" label="Toggle sidebar" />
          <IconButton icon="stop" label={t.stop} variant="secondary" />
          <Menu
            label={t.moreActions}
            onAction={setLast}
            items={[
              { id: 'rename', label: t.rename, icon: 'file' },
              { id: 'pin', label: t.pin, icon: 'check' },
              { id: 'delete', label: t.delete, danger: true, separatorBefore: true },
            ]}
          />
          <ConfirmDialog
            trigger={<Button variant="secondary">{t.delete}…</Button>}
            title={t.deleteSession}
            body={t.deleteSessionBody}
            confirmLabel={t.delete}
            cancelLabel={t.cancel}
            danger
            onConfirm={() => setLast('confirmed')}
          />
          <span className={s.caption} aria-live="polite">
            {last}
          </span>
        </div>
      </section>

      <section>
        <h2>Fields</h2>
        <div className={s.grid}>
          <SearchField label={t.search} clearLabel={t.clearSearch} value={query} onChange={setQuery} />
          <TextField label={t.accountName} placeholder="Work" value={name} onChange={setName} isInvalid={name.length > 80} errorMessage={t.accountNameError} />
          <TextField label={t.accountName} defaultValue={'x'.repeat(81)} isInvalid errorMessage={t.accountNameError} />
          <TextField label={t.accountName} defaultValue="Personal" isDisabled />
          <TextField label={t.composerPlaceholder} multiline placeholder={t.composerPlaceholder} />
          <div className={s.stack}>
            <Switch isSelected={verify} onChange={setVerify}>
              {t.verifyChanges}
            </Switch>
            <span className={s.caption}>{t.verifyChangesHelp}</span>
            <Switch isDisabled>{t.access.autopilot}</Switch>
          </div>
        </div>
      </section>

      <section>
        <h2>Status</h2>
        <div className={s.row}>
          <Badge tone="accent">{t.needsYouShort}</Badge>
          <Badge tone="neutral" icon="shield">
            {t.access['read-only']}
          </Badge>
          <Badge tone="success" icon="check">
            {t.checksPassed(128)}
          </Badge>
          <Badge tone="warning" icon="alert">
            {t.interrupted}
          </Badge>
          <Badge tone="danger" icon="alert">
            {t.reject}
          </Badge>
        </div>
      </section>

      <section>
        <h2>Formats</h2>
        <dl className={s.formats}>
          <dt>Number</dt>
          <dd>{formatNumber(locale, 1234567.5)}</dd>
          <dt>Quota</dt>
          <dd>
            {formatPercent(locale, 72.4)} · {formatPercent(locale, null)}
          </dd>
          <dt>Reset</dt>
          <dd>
            {formatRelative(locale, now + 3 * 3600_000, now)} · {formatTime(locale, now + 3 * 3600_000)}
          </dd>
          <dt>Updated</dt>
          <dd>{formatRelative(locale, now - 5 * 60_000, now)}</dd>
        </dl>
      </section>

      <section>
        <h2>Icons</h2>
        <div className={s.icons}>
          {iconNames.map((n) => (
            <span key={n} title={n}>
              <Icon name={n} size={20} />
              <small>{n}</small>
            </span>
          ))}
        </div>
      </section>

      <section>
        <h2>Type</h2>
        <p className={s.xxl}>lumen</p>
        <p className={s.xl}>{t.deleteSession}</p>
        <p className={s.body}>{t.deleteSessionBody}</p>
        <p className={s.small}>{t.verifyChangesHelp}</p>
        <code className={s.mono}>cargo test -p lumen-gatekeeper</code>
      </section>
    </main>
  );
}
