import { useState } from 'react';
import { call } from '../lib/bridge';
import { errorText } from '../lib/util';
import { useI18n } from '../lib/i18n';
export default function FileDiff({ folder, file }: { folder: string; file: string }) {
  const { t } = useI18n();
  const [data, setData] = useState<{ text: string; truncated: boolean } | null>(null);
  const [error, setError] = useState('');
  return (
    <details
      className="file-diff"
      onToggle={(e) => {
        if (!e.currentTarget.open || data) return;
        void call<{ text: string; truncated: boolean }>('file_change_diff', { folder, file })
          .then(setData)
          .catch((e) => setError(errorText(e)));
      }}
    >
      <summary>
        {t('View changes')}: {file}
      </summary>
      {error ? (
        <p role="alert">{error}</p>
      ) : data ? (
        <>
          <pre aria-label={`${t('Changes in')} ${file}`}>{data.text || t('No text changes.')}</pre>
          {data.truncated && (
            <small>{t('Preview truncated. Open the file to review all changes.')}</small>
          )}
        </>
      ) : (
        <p role="status">{t('Loading changes…')}</p>
      )}
    </details>
  );
}
