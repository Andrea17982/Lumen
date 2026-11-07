import { useEffect, useState } from 'react';
import { call } from '../lib/bridge';

export default function AttachmentPreview({ folder, path }: { folder: string; path: string }) {
  const [src, setSrc] = useState('');
  useEffect(() => {
    let disposed = false;
    void call<string>('attachment_image_data', { folder, path })
      .then((data) => {
        if (!disposed) setSrc(data);
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, [folder, path]);
  return src ? (
    <img
      className="message-image-attachment"
      src={src}
      alt={path.split(/[\\/]/).at(-1) || 'Attached image'}
    />
  ) : null;
}
