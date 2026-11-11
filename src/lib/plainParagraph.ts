/** Return literal paragraph text only for a deliberately narrow subset of CommonMark/GFM.
 * Undefined always delegates to the existing parser. Never returns generated HTML.
 */
export function plainParagraph(text: string): string | undefined {
  if (/[\r\n\t\u0000-\u001f\u007f\ufeff\u2028\u2029]/.test(text)) return;
  if (/[*~`<>\\[\]#|&@]/.test(text)) return;
  if (/(^|[^A-Za-z0-9_])_|_($|[^A-Za-z0-9_])/.test(text)) return;
  if (text.includes('://') || /www\./i.test(text)) return;
  if (/^ {4}|^ {0,3}(?:[-+](?:\s|$)|\d+[.)](?:\s|$))/.test(text)) return;
  if (/^ {0,3}(?:-\s*){3,}$/.test(text)) return;
  let start = 0;
  while (start < 3 && text.charCodeAt(start) === 32) start++;
  let end = text.length;
  while (end > start && text.charCodeAt(end - 1) === 32) end--;
  return text.slice(start, end);
}
