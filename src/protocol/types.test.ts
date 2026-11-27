import { describe, expect, it } from 'vitest';
import { isCancelled } from './types';

describe('isCancelled', () => {
  it('recognises the daemon answer however the transport carries it', () => {
    expect(isCancelled('Cancelled')).toBe(true);
    expect(isCancelled(new Error('Cancelled'))).toBe(true);
    expect(isCancelled(' Cancelled\n')).toBe(true);
  });
  it('does not take other failures for a cancel', () => {
    expect(isCancelled('Cancelled by quota')).toBe(false);
    expect(isCancelled(new Error('No account is currently available'))).toBe(false);
    expect(isCancelled(undefined)).toBe(false);
  });
});
