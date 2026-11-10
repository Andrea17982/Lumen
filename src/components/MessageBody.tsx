import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { memo, useState } from 'react';
import { call } from '../lib/bridge';
import { errorText } from '../lib/util';
import ResponseImage from './ResponseImage';

function MessageBody({ text, folder }: { text: string; folder?: string }) {
  const [error, setError] = useState('');
  return (
    <>
      {error && <p role="alert">{error}</p>}
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        urlTransform={(url) => url}
        components={{
          a: ({ children, href, node }) =>
            node?.children.some((child) => child.type === 'element' && child.tagName === 'img') ? (
              <span>{children}</span>
            ) : (
              <button
                type="button"
                className="response-link"
                title={href}
                onClick={() => {
                  setError('');
                  void call('open_response_link', { folder: folder || null, href }).catch((e) =>
                    setError(errorText(e)),
                  );
                }}
              >
                {children}
              </button>
            ),
          img: ({ alt, src }) =>
            typeof src === 'string' ? (
              <ResponseImage href={src} alt={alt || ''} folder={folder} />
            ) : null,
        }}
      >
        {text}
      </ReactMarkdown>
    </>
  );
}
export default memo(MessageBody);
