import { useLocale, useT } from '../i18n';
import { Mark } from '../brand/Logo';
import { code, demo } from './demo';
import { Icon } from '../design/icons';
import s from './AppMock.module.css';

export function AppMock() {
  const t = useT();
  const d = demo[useLocale()];
  return (
    <div className={s.window}>
      <aside className={s.sidebar} aria-label={t.sessions}>
        <div className={s.brandRow}>
          <Mark size={26} />
        </div>
        <button className={s.newTask}>
          <Icon name="plus" />
          {t.newTask}
        </button>
        <div className={s.search} role="search">
          <Icon name="search" size={14} />
          <span>{t.search}</span>
          <kbd>Ctrl K</kbd>
        </div>
        <nav className={s.sessions}>
          <h2>{t.today}</h2>
          <a className={s.session} aria-current="page">
            <span className={s.spinner} role="img" aria-label={t.running} />
            <span className={s.title}>{d.sessionsToday[0]}</span>
          </a>
          <a className={s.session}>
            <span className={s.needsDot} aria-hidden="true" />
            <span className={s.title}>{d.sessionsToday[1]}</span>
            <span className={s.badge} title={t.needsYou}>{t.needsYouShort}</span>
          </a>
          <a className={s.session}>
            <span className={s.idleDot} aria-hidden="true" />
            <span className={s.title}>{d.sessionsToday[2]}</span>
          </a>
          <h2>{t.yesterday}</h2>
          <a className={s.session}>
            <span className={s.idleDot} aria-hidden="true" />
            <span className={s.title}>{d.sessionsYesterday[0]}</span>
          </a>
          <a className={s.session}>
            <span className={s.warnDot} role="img" aria-label={t.interrupted} />
            <span className={s.title}>{d.sessionsYesterday[1]}</span>
          </a>
        </nav>
        <div className={s.project}>
          <Icon name="folder" />
          <div>
            <strong>{d.project}</strong>
            <small>{d.path}</small>
          </div>
        </div>
      </aside>

      <main className={s.main}>
        <header className={s.header}>
          <h1>{d.title}</h1>
          <span className={s.pill}>
            <Icon name="shield" size={13} />
            {t.access['file-edits']}
          </span>
        </header>

        <div className={s.transcript} role="log" aria-label={t.conversation}>
          <article className={s.user} aria-label={t.you}>
            <p>{d.user}</p>
          </article>

          <article className={s.assistant} aria-label={t.appName}>
            <div className={s.route}>
              <Mark size={16} label="" />
              {t.routedTo(d.model)}
            </div>
            <p>{d.answer1}</p>
            <pre>
              <code>{code}</code>
            </pre>
            <details className={s.activity}>
              <summary>
                <Icon name="chevron" size={14} />
                <span>{t.activities(6)}</span>
                <span className={s.sep}>·</span>
                <Icon name="terminal" size={14} />
                <span>{t.ranCommand('cargo test -p lumen-gatekeeper')}</span>
                <span className={s.sep}>·</span>
                <Icon name="file" size={14} />
                <span>{t.editedFiles(3)}</span>
                <span className={s.ok}>
                  <Icon name="check" size={14} />
                  {t.checksPassed(128)}
                </span>
              </summary>
            </details>
            <p>{d.answer2}</p>
          </article>

          <section className={s.approval} aria-label={t.wantsToRun('Codex')}>
            <div className={s.approvalHead}>
              <Icon name="terminal" />
              <strong>{t.wantsToRun('Codex')}</strong>
            </div>
            <code className={s.command}>{d.command}</code>
            <p className={s.reason}>
              {d.reason} <span>· {d.cwd}</span>
            </p>
            <div className={s.actions}>
              <button className={s.secondary}>{t.reject}</button>
              <button className={s.primary}>{t.approve}</button>
            </div>
          </section>

          <article className={s.assistant} aria-label={t.writing}>
            <div className={s.route}>
              <span className={s.spinner} />
              {t.writing}
            </div>
            <p>
              {d.streaming}
              <span className={s.caret} aria-hidden="true" />
            </p>
          </article>
        </div>

        <footer className={s.composer}>
          <div className={s.field}>
            <textarea rows={2} placeholder={t.composerPlaceholder} aria-label={t.composerPlaceholder} />
            <div className={s.fieldBar}>
              <span className={s.chip}>{t.modeAuto}</span>
              <span className={s.chip}>{t.access['file-edits']}</span>
              <button className={s.send} aria-label={t.send}>
                <Icon name="up" size={16} />
              </button>
            </div>
          </div>
        </footer>
      </main>
    </div>
  );
}
