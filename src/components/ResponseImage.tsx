import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Image as ImageIcon, X } from 'lucide-react';
import { call } from '../lib/bridge';
import { errorText } from '../lib/util';
import { useI18n } from '../lib/i18n';

export default function ResponseImage({
  href,
  alt,
  folder,
}: {
  href: string;
  alt: string;
  folder?: string;
}) {
  const { t } = useI18n();
  const remote = /^https?:\/\//i.test(href);
  const [src, setSrc] = useState('');
  const [error, setError] = useState('');
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    let active = true;
    setSrc('');
    setError('');
    if (!remote && folder) {
      void call<string>('response_image_data', { folder, href })
        .then((data) => {
          if (active) setSrc(data);
        })
        .catch((e) => {
          if (active) setError(errorText(e));
        });
    }
    return () => {
      active = false;
    };
  }, [folder, href, remote]);
  if (!src)
    return (
      <span className="response-image-placeholder">
        <ImageIcon size={16} aria-hidden="true" />
        {remote ? (
          <button className="text-button" onClick={() => setSrc(href)}>
            {t('Load image')}: {alt || href}
          </button>
        ) : (
          <span>{alt || href}</span>
        )}
        {error && <small role="status">{error}</small>}
      </span>
    );
  return (
    <span className="response-image">
      <button
        className="response-image-open"
        aria-label={`${t('Enlarge image')}: ${alt}`}
        onClick={() => dialog.current?.showModal()}
      >
        <img
          src={src}
          alt={alt}
          loading="lazy"
          referrerPolicy="no-referrer"
          onError={() => {
            setSrc('');
            setError(t('The image could not be loaded.'));
          }}
        />
      </button>
      {createPortal(
        <dialog
          ref={dialog}
          className="response-image-dialog"
          aria-label={alt || t('Image preview')}
          onClick={(e) => {
            if (e.target === e.currentTarget) e.currentTarget.close();
          }}
        >
          <button
            className="icon-button"
            aria-label={t('Close image')}
            onClick={() => dialog.current?.close()}
          >
            <X size={20} />
          </button>
          <img src={src} alt={alt} referrerPolicy="no-referrer" />
          {alt && <p>{alt}</p>}
        </dialog>,
        document.body,
      )}
    </span>
  );
}
