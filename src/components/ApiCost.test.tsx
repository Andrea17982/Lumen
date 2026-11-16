import { renderToStaticMarkup } from 'react-dom/server';
import { describe, it, expect } from 'vitest';
import ApiCost from './ApiCost';
describe('API cost estimates', () => {
  it('replaces reservations with usage per request, without double counting', () => {
    const activity = [
      { kind: 'api-cost', text: JSON.stringify({ requestId: 'a', estimatedUsd: 1 }) },
      { kind: 'api-cost', text: JSON.stringify({ requestId: 'a', estimatedUsd: 0.25 }) },
      { kind: 'api-cost', text: JSON.stringify({ requestId: 'b', estimatedUsd: 0.5 }) },
      { kind: 'api-cost', text: 'broken' },
    ];
    expect(renderToStaticMarkup(<ApiCost activity={activity} />)).toContain('$0.750000');
  });
  it('does not invent a price when no valid cost is present', () => {
    expect(
      renderToStaticMarkup(
        <ApiCost
          activity={[
            { kind: 'api-cost', text: JSON.stringify({ requestId: 'x', estimatedUsd: -1 }) },
          ]}
        />,
      ),
    ).toBe('');
  });
});
