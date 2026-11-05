import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export default function MessageBody({ text }: { text: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        a: ({ children, href }) => (
          <span title={href}>
            {children} <span className="link-target">({href})</span>
          </span>
        ),
        img: ({ alt }) => <span>[Image: {alt}]</span>,
      }}
    >
      {text}
    </ReactMarkdown>
  );
}
